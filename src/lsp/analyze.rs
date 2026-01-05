use crate::cache::{is_cache, set_cache_path};
use crate::models::Workspace;
use crate::toolchain;
use anyhow::bail;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process,
    sync::{Notify, mpsc},
};

#[derive(serde::Deserialize, Clone, Debug)]
pub struct CargoCheckMessageTarget {
    name: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum CargoCheckMessage {
    CompilerArtifact {
        target: CargoCheckMessageTarget,
    },
    #[allow(unused)]
    BuildFinished {},
}

pub enum AnalyzerEvent {
    CrateChecked {
        package: String,
        package_index: usize,
        package_count: usize,
    },
    Analyzed(Workspace),
}

#[derive(Clone, Debug)]
pub struct Analyzer {
    path: PathBuf,
    metadata: Option<cargo_metadata::Metadata>,
    rustc_threads: usize,
}

impl Analyzer {
    pub async fn new(path: impl AsRef<Path>, rustc_threads: usize) -> crate::error::Result<Self> {
        let path = path.as_ref().to_path_buf();

        let mut cargo_cmd = toolchain::setup_cargo_command(rustc_threads).await;
        cargo_cmd
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTC_WRAPPER")
            // `--config` values are TOML; `""` sets the wrapper to an empty string.
            .args([
                "--config",
                "build.rustc-wrapper=\"\"",
                "--config",
                "build.rustc-workspace-wrapper=\"\"",
                "metadata",
                "--format-version",
                "1",
                "--filter-platform",
                toolchain::HOST_TUPLE,
            ])
            .current_dir(if path.is_file() {
                path.parent().unwrap()
            } else {
                &path
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let metadata = match cargo_cmd.output().await {
            Ok(output) if output.status.success() => {
                let data = String::from_utf8_lossy(&output.stdout);
                cargo_metadata::MetadataCommand::parse(data).ok()
            }
            Ok(output) => {
                if tracing::enabled!(tracing::Level::DEBUG) {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::debug!(
                        "`cargo metadata` failed (status={}):\nstdout:\n{}\nstderr:\n{}",
                        output.status,
                        stdout.trim(),
                        stderr.trim()
                    );
                }
                None
            }
            Err(e) => {
                if tracing::enabled!(tracing::Level::DEBUG) {
                    tracing::debug!("failed to spawn `cargo metadata`: {e}");
                }
                None
            }
        };

        if let Some(metadata) = metadata {
            Ok(Self {
                path: metadata.workspace_root.as_std_path().to_path_buf(),
                metadata: Some(metadata),
                rustc_threads,
            })
        } else if path.is_file() && path.extension().map(|v| v == "rs").unwrap_or(false) {
            Ok(Self {
                path,
                metadata: None,
                rustc_threads,
            })
        } else {
            tracing::error!("Invalid analysis target: {}", path.display());
            bail!("Invalid analysis target: {}", path.display());
        }
    }
    pub fn target_path(&self) -> &Path {
        &self.path
    }
    pub fn workspace_path(&self) -> Option<&Path> {
        if self.metadata.is_some() {
            Some(&self.path)
        } else {
            None
        }
    }

    pub async fn analyze(&self, all_targets: bool, all_features: bool) -> AnalyzeEventIter {
        if let Some(metadata) = &self.metadata
            && metadata.root_package().is_some()
        {
            self.analyze_package(metadata, all_targets, all_features)
                .await
        } else {
            self.analyze_single_file(&self.path).await
        }
    }

    async fn analyze_package(
        &self,
        metadata: &cargo_metadata::Metadata,
        all_targets: bool,
        all_features: bool,
    ) -> AnalyzeEventIter {
        let package_name = metadata.root_package().as_ref().unwrap().name.to_string();
        let target_dir = metadata.target_directory.as_std_path().join("owl");
        tracing::debug!("clear cargo cache");
        let mut command = toolchain::setup_cargo_command(self.rustc_threads).await;
        command
            .args(["clean", "--package", &package_name])
            .env("CARGO_TARGET_DIR", &target_dir)
            .current_dir(&self.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn().unwrap().wait().await.ok();

        let mut command = toolchain::setup_cargo_command(self.rustc_threads).await;

        let mut args = vec!["check", "--workspace"];
        if all_targets {
            args.push("--all-targets");
        }
        if all_features {
            args.push("--all-features");
        }
        args.extend_from_slice(&["--keep-going", "--message-format=json"]);

        command
            .args(args)
            .env("CARGO_TARGET_DIR", &target_dir)
            .env_remove("RUSTC_WRAPPER")
            .current_dir(&self.path)
            .stdout(Stdio::piped())
            .kill_on_drop(true);

        if is_cache() {
            set_cache_path(&mut command, target_dir);
        }

        if !tracing::enabled!(tracing::Level::INFO) {
            command.stderr(Stdio::null());
        }

        // Cargo emits `compiler-artifact` per compilation unit. `metadata.packages[*].targets`
        // includes lots of targets Cargo won't build for `cargo check` (tests/benches/examples,
        // and dependency binaries), which can wildly overcount.
        //
        // We estimate the total units Cargo will actually build:
        // - Workspace members: lib/bin/proc-macro/custom-build; plus test/bench/example with --all-targets
        // - Dependencies: lib/proc-macro/custom-build only
        let workspace_members: HashSet<_> = metadata.workspace_members.iter().cloned().collect();

        let package_count = metadata
            .packages
            .iter()
            .map(|p| {
                let is_workspace_member = workspace_members.contains(&p.id);
                p.targets
                    .iter()
                    .filter(|t| {
                        let always = t.is_lib()
                            || t.is_proc_macro()
                            || t.is_custom_build()
                            || (is_workspace_member && t.is_bin());
                        let extra = all_targets
                            && is_workspace_member
                            && (t.is_test() || t.is_bench() || t.is_example());
                        always || extra
                    })
                    .count()
            })
            .sum::<usize>()
            .max(1);

        tracing::debug!("start analyzing package {package_name}");
        let mut child = command.spawn().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());

        let (sender, receiver) = mpsc::channel(1024);
        let notify = Arc::new(Notify::new());
        let notify_c = notify.clone();
        let _handle = tokio::spawn(async move {
            // prevent command from dropped
            let mut checked_count = 0usize;

            // Cargo emits JSON objects tagged with `{"reason": ...}`.
            // rustowlc emits a serialized `Workspace` JSON object.
            //
            // Distinguish them by attempting to parse any line as a `Workspace` first.
            // If that fails, treat it as a cargo message (and optionally parse progress from it).

            let mut buf = Vec::with_capacity(16 * 1024);
            loop {
                buf.clear();
                match stdout.read_until(b'\n', &mut buf).await {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }

                // Trim trailing newline(s) to keep serde_json happy.
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                if buf.is_empty() {
                    continue;
                }

                if let Ok(ws) = serde_json::from_slice::<Workspace>(&buf) {
                    let event = AnalyzerEvent::Analyzed(ws);
                    let _ = sender.send(event).await;
                    continue;
                }

                // Not a Workspace line; maybe a Cargo JSON message.
                if let Ok(CargoCheckMessage::CompilerArtifact { target }) =
                    serde_json::from_slice::<CargoCheckMessage>(&buf)
                {
                    let checked = target.name;
                    tracing::trace!("crate {checked} checked");

                    checked_count = checked_count.saturating_add(1);
                    let event = AnalyzerEvent::CrateChecked {
                        package: checked,
                        package_index: checked_count,
                        package_count,
                    };
                    let _ = sender.send(event).await;
                }
            }

            tracing::debug!("stdout closed");
            notify_c.notify_one();
        });

        AnalyzeEventIter {
            receiver,
            notify,
            child,
        }
    }

    async fn analyze_single_file(&self, path: &Path) -> AnalyzeEventIter {
        let sysroot = toolchain::get_sysroot().await;
        let rustowlc_path = toolchain::get_executable_path("rustowlc").await;

        let mut command = process::Command::new(&rustowlc_path);
        command
            .arg(&rustowlc_path) // rustowlc triggers when first arg is the path of itself
            .arg(format!("--sysroot={}", sysroot.display()))
            .arg("--crate-type=lib");
        #[cfg(unix)]
        command.arg("-o/dev/null");
        #[cfg(windows)]
        command.arg("-oNUL");
        command.arg(path).stdout(Stdio::piped()).kill_on_drop(true);

        toolchain::set_rustc_env(&mut command, &sysroot);

        // When running under `cargo llvm-cov`, ensure the rustowlc subprocess writes its
        // coverage somewhere cargo-llvm-cov will pick up.
        if let Ok(profile_file) = std::env::var("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile_file);
        }

        if !tracing::enabled!(tracing::Level::INFO) {
            command.stderr(Stdio::null());
        }

        tracing::debug!("start analyzing {}", path.display());
        let mut child = command.spawn().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());

        let (sender, receiver) = mpsc::channel(1024);
        let notify = Arc::new(Notify::new());
        let notify_c = notify.clone();
        let _handle = tokio::spawn(async move {
            // prevent command from dropped

            let mut buf = Vec::with_capacity(16 * 1024);
            loop {
                buf.clear();
                match stdout.read_until(b'\n', &mut buf).await {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }

                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                if buf.is_empty() {
                    continue;
                }

                if let Ok(ws) = serde_json::from_slice::<Workspace>(&buf) {
                    let event = AnalyzerEvent::Analyzed(ws);
                    let _ = sender.send(event).await;
                }
            }

            tracing::debug!("stdout closed");
            notify_c.notify_one();
        });

        AnalyzeEventIter {
            receiver,
            notify,
            child,
        }
    }
}

pub struct AnalyzeEventIter {
    receiver: mpsc::Receiver<AnalyzerEvent>,
    notify: Arc<Notify>,
    #[allow(unused)]
    child: process::Child,
}
impl AnalyzeEventIter {
    pub async fn next_event(&mut self) -> Option<AnalyzerEvent> {
        tokio::select! {
            v = self.receiver.recv() => v,
            _ = self.notify.notified() => {
                match self.child.wait().await {
                    Ok(status) => {
                        if !status.success() {
                            tracing::debug!("Analyzer process exited with status: {}", status);
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Failed to wait for analyzer process: {}", e);
                    }
                }
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miri_async_test;

    #[test]
    fn new_accepts_single_rust_file_and_has_no_workspace_path() {
        miri_async_test!(async {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("main.rs");
            std::fs::write(&target, "fn main() {}\n").unwrap();

            let analyzer = Analyzer::new(&target, 1).await.unwrap();
            assert_eq!(analyzer.target_path(), target.as_path());
            assert_eq!(analyzer.workspace_path(), None);
        });
    }

    #[test]
    fn new_rejects_invalid_paths() {
        miri_async_test!(async {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("not_a_rust_project");
            std::fs::create_dir_all(&target).unwrap();

            let err = Analyzer::new(&target, 1).await.unwrap_err();
            assert!(err.to_string().contains("Invalid analysis target"));
        });
    }

    #[test]
    fn analyze_single_file_yields_analyzed_event() {
        miri_async_test!(async {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("lib.rs");
            std::fs::write(&target, "pub fn f() -> i32 { 1 }\n").unwrap();

            let analyzer = Analyzer::new(&target, 1).await.unwrap();
            let mut iter = analyzer.analyze(false, false).await;

            // Wait for an `Analyzed` event; otherwise fail with some context.
            let mut saw_crate_checked = false;
            for _ in 0..50 {
                match iter.next_event().await {
                    Some(AnalyzerEvent::CrateChecked { .. }) => {
                        saw_crate_checked = true;
                    }
                    Some(AnalyzerEvent::Analyzed(ws)) => {
                        // Workspace emitted by rustowlc should be serializable and non-empty.
                        // We at least expect it to include this file name somewhere.
                        let json = serde_json::to_string(&ws).unwrap();
                        assert!(json.contains("lib.rs"));
                        return;
                    }
                    None => break,
                }
            }

            panic!(
                "did not receive AnalyzerEvent::Analyzed (saw_crate_checked={})",
                saw_crate_checked
            );
        });
    }
}
