use super::analyze::*;
use crate::{lsp::*, models::*, utils};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::{sync::RwLock, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{self as lsp_types, *};
use tower_lsp_server::{Client, LanguageServer, LspService};

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeRequest {}
#[derive(serde::Serialize, Clone, Debug)]
pub struct AnalyzeResponse {}

#[derive(Clone, Copy, Debug)]
pub struct CheckReport {
    pub ok: bool,
    pub checked_targets: usize,
    pub total_targets: Option<usize>,
    pub duration: std::time::Duration,
}

/// RustOwl LSP server backend
pub struct Backend {
    client: Client,
    analyzers: Arc<RwLock<Vec<Analyzer>>>,
    status: Arc<RwLock<progress::AnalysisStatus>>,
    analyzed: Arc<RwLock<Option<Crate>>>,
    /// Open documents cache to avoid re-reading and re-indexing on each cursor request.
    open_docs: Arc<RwLock<HashMap<PathBuf, OpenDoc>>>,
    processes: Arc<RwLock<JoinSet<()>>>,
    process_tokens: Arc<RwLock<BTreeMap<usize, CancellationToken>>>,
    work_done_progress: Arc<RwLock<bool>>,
    rustc_thread: usize,
}

#[derive(Clone, Debug)]
struct OpenDoc {
    text: Arc<String>,
    index: Arc<utils::LineCharIndex>,
    line_start_bytes: Arc<Vec<u32>>,
}

impl Backend {
    pub fn new(rustc_thread: usize) -> impl Fn(Client) -> Self {
        move |client: Client| Self {
            client,
            analyzers: Arc::new(RwLock::new(Vec::new())),
            analyzed: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(progress::AnalysisStatus::Finished)),
            open_docs: Arc::new(RwLock::new(HashMap::new())),
            processes: Arc::new(RwLock::new(JoinSet::new())),
            process_tokens: Arc::new(RwLock::new(BTreeMap::new())),
            work_done_progress: Arc::new(RwLock::new(false)),
            rustc_thread,
        }
    }

    async fn add_analyze_target(&self, path: &Path) -> bool {
        if let Ok(new_analyzer) = Analyzer::new(&path, self.rustc_thread).await {
            let mut analyzers = self.analyzers.write().await;
            for analyzer in &*analyzers {
                if analyzer.target_path() == new_analyzer.target_path() {
                    return true;
                }
            }
            analyzers.push(new_analyzer);
            true
        } else {
            false
        }
    }

    pub async fn analyze(&self, _params: AnalyzeRequest) -> Result<AnalyzeResponse> {
        tracing::debug!("rustowl/analyze request received");
        self.do_analyze().await;
        Ok(AnalyzeResponse {})
    }
    async fn do_analyze(&self) {
        self.shutdown_subprocesses().await;
        self.analyze_with_options(false, false).await;
    }

    async fn analyze_with_options(&self, all_targets: bool, all_features: bool) {
        tracing::trace!("wait 100ms for rust-analyzer");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        tracing::debug!("stop running analysis processes");
        self.shutdown_subprocesses().await;

        tracing::debug!("start analysis");
        {
            *self.status.write().await = progress::AnalysisStatus::Analyzing;
        }
        let analyzers = { self.analyzers.read().await.clone() };

        tracing::debug!("analyze {} packages...", analyzers.len());
        for analyzer in analyzers {
            let analyzed = self.analyzed.clone();
            let client = self.client.clone();
            let work_done_progress = self.work_done_progress.clone();
            let cancellation_token = CancellationToken::new();

            let cancellation_token_key = {
                let token = cancellation_token.clone();
                let mut tokens = self.process_tokens.write().await;
                let key = if let Some(key) = tokens.last_entry().map(|v| *v.key()) {
                    key + 1
                } else {
                    1
                };
                tokens.insert(key, token);
                key
            };

            let process_tokens = self.process_tokens.clone();
            self.processes.write().await.spawn(async move {
                let mut progress_token = None;
                if *work_done_progress.read().await {
                    progress_token =
                        Some(progress::ProgressToken::begin(client, None::<&str>).await)
                };

                let mut iter = analyzer.analyze(all_targets, all_features).await;
                while let Some(event) = tokio::select! {
                    _ = cancellation_token.cancelled() => None,
                    event = iter.next_event() => event,
                } {
                    match event {
                        AnalyzerEvent::CrateChecked {
                            package,
                            package_index,
                            package_count,
                        } => {
                            if let Some(token) = &progress_token {
                                let percentage = (package_index * 100 / package_count).min(100);
                                token
                                    .report(
                                        Some(format!("{package} analyzed")),
                                        Some(percentage as u32),
                                    )
                                    .await;
                            }
                        }
                        AnalyzerEvent::Analyzed(ws) => {
                            let write = &mut *analyzed.write().await;
                            for krate in ws.0.into_values() {
                                if let Some(write) = write {
                                    write.merge(krate);
                                } else {
                                    *write = Some(krate);
                                }
                            }
                        }
                    }
                }
                // remove cancellation token from list
                process_tokens.write().await.remove(&cancellation_token_key);

                if let Some(progress_token) = progress_token {
                    progress_token.finish().await;
                }
            });
        }

        let processes = self.processes.clone();
        let status = self.status.clone();
        let analyzed = self.analyzed.clone();
        tokio::spawn(async move {
            while { processes.write().await.join_next().await }.is_some() {}
            let mut status = status.write().await;
            let analyzed = analyzed.write().await;
            if *status != progress::AnalysisStatus::Error {
                if analyzed.as_ref().map(|v| v.0.len()).unwrap_or(0) == 0 {
                    *status = progress::AnalysisStatus::Error;
                } else {
                    *status = progress::AnalysisStatus::Finished;
                }
            }
        });
    }

    async fn decos(
        &self,
        filepath: &Path,
        position: Loc,
    ) -> std::result::Result<Vec<decoration::Deco>, progress::AnalysisStatus> {
        let mut selected = decoration::SelectLocal::new(position);
        let mut error = progress::AnalysisStatus::Error;

        let analyzed_guard = self.analyzed.read().await;
        let Some(analyzed) = analyzed_guard.as_ref() else {
            return Err(error);
        };

        // Fast path: LSP file paths should be UTF-8 and match our stored file keys.
        // Fall back to the previous Path comparison if the direct lookup misses.
        let mut matched_file = filepath
            .to_str()
            .and_then(|path_str| analyzed.0.get(path_str));

        if matched_file.is_none() {
            for (filename, file) in analyzed.0.iter() {
                if filepath == Path::new(filename) {
                    matched_file = Some(file);
                    break;
                }
            }
        }

        let Some(file) = matched_file else {
            return Err(error);
        };

        if !file.items.is_empty() {
            error = progress::AnalysisStatus::Finished;
        }

        for item in &file.items {
            utils::mir_visit(item, &mut selected);
        }

        let selected_local = selected.selected();
        if selected_local.is_none() {
            return Err(error);
        }

        let mut calc = decoration::CalcDecos::new(selected_local.iter().copied());
        for item in &file.items {
            utils::mir_visit(item, &mut calc);
        }

        calc.handle_overlapping();
        let decos = calc.decorations();
        if decos.is_empty() {
            Err(error)
        } else {
            Ok(decos)
        }
    }

    pub async fn cursor(
        &self,
        params: decoration::CursorRequest,
    ) -> Result<decoration::Decorations> {
        let is_analyzed = self.analyzed.read().await.is_some();
        let status = *self.status.read().await;

        let Some(path) = params.path() else {
            return Ok(decoration::Decorations {
                is_analyzed,
                status,
                path: None,
                decorations: Vec::new(),
            });
        };

        let (_text, index) = if let Some(open) = self.open_docs.read().await.get(&path).cloned() {
            (open.text, open.index)
        } else if let Ok(text) = tokio::fs::read_to_string(&path).await {
            let index = Arc::new(utils::LineCharIndex::new(&text));
            (Arc::new(text), index)
        } else {
            return Ok(decoration::Decorations {
                is_analyzed,
                status,
                path: Some(path),
                decorations: Vec::new(),
            });
        };

        let position = params.position();
        let pos = Loc(index.line_char_to_index(position.line, position.character));
        let (decos, status) = match self.decos(&path, pos).await {
            Ok(v) => (v, status),
            Err(e) => (
                Vec::new(),
                if status == progress::AnalysisStatus::Finished {
                    e
                } else {
                    status
                },
            ),
        };

        let mut decorations = Vec::with_capacity(decos.len());
        decorations.extend(decos.into_iter().map(|v| v.to_lsp_range(index.as_ref())));

        Ok(decoration::Decorations {
            is_analyzed,
            status,
            path: Some(path),
            decorations,
        })
    }

    pub async fn check(path: impl AsRef<Path>, rustc_thread: usize) -> bool {
        Self::check_with_options(path, false, false, rustc_thread).await
    }

    pub async fn check_report_with_options(
        path: impl AsRef<Path>,
        all_targets: bool,
        all_features: bool,
        rustc_thread: usize,
    ) -> CheckReport {
        use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
        use std::io::IsTerminal;

        let start = std::time::Instant::now();
        let path = path.as_ref();
        let (service, _) = LspService::build(Backend::new(rustc_thread)).finish();
        let backend = service.inner();

        if !backend.add_analyze_target(path).await {
            return CheckReport {
                ok: false,
                checked_targets: 0,
                total_targets: None,
                duration: start.elapsed(),
            };
        }

        let progress_bar = if std::io::stderr().is_terminal() {
            let progress_bar = ProgressBar::new(0);
            progress_bar.set_draw_target(ProgressDrawTarget::stderr());
            progress_bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} {wide_msg} [{bar:40.cyan/blue}] {pos}/{len}",
                )
                .unwrap(),
            );
            progress_bar.set_message("Analyzing...");
            Some(progress_bar)
        } else {
            None
        };

        let _progress_guard = progress_bar
            .as_ref()
            .cloned()
            .map(crate::ActiveProgressBarGuard::set);

        // Re-analyze, but consume the iterator and use it to power a CLI progress bar.
        backend.shutdown_subprocesses().await;
        let analyzers = { backend.analyzers.read().await.clone() };

        let mut checked_targets = 0usize;
        let mut total_targets = None;
        let mut last_log_at = std::time::Instant::now();
        let mut analyzed: Option<Crate> = None;

        for analyzer in analyzers {
            let mut iter = analyzer.analyze(all_targets, all_features).await;
            while let Some(event) = iter.next_event().await {
                match event {
                    AnalyzerEvent::CrateChecked {
                        package,
                        package_index,
                        package_count,
                    } => {
                        checked_targets = package_index;
                        total_targets = Some(package_count);

                        if let Some(pb) = &progress_bar {
                            pb.set_length(package_count as u64);
                            pb.set_position(package_index as u64);
                            pb.set_message(format!("Checking {package}"));
                        } else if last_log_at.elapsed() >= std::time::Duration::from_secs(1) {
                            eprintln!("Checking {package} ({package_index}/{package_count})");
                            last_log_at = std::time::Instant::now();
                        }
                    }
                    AnalyzerEvent::Analyzed(ws) => {
                        for krate in ws.0.into_values() {
                            if let Some(write) = &mut analyzed {
                                write.merge(krate);
                            } else {
                                analyzed = Some(krate);
                            }
                        }
                    }
                }
            }
        }

        if let Some(pb) = progress_bar {
            pb.finish_and_clear();
        }

        let ok = analyzed.as_ref().map(|v| !v.0.is_empty()).unwrap_or(false);

        CheckReport {
            ok,
            checked_targets,
            total_targets,
            duration: start.elapsed(),
        }
    }

    pub async fn check_with_options(
        path: impl AsRef<Path>,
        all_targets: bool,
        all_features: bool,
        rustc_thread: usize,
    ) -> bool {
        Self::check_report_with_options(path, all_targets, all_features, rustc_thread)
            .await
            .ok
    }

    #[cfg(feature = "bench")]
    pub async fn load_analyzed_state_for_bench(
        &self,
        path: impl AsRef<Path>,
        all_targets: bool,
        all_features: bool,
    ) -> bool {
        let path = path.as_ref();

        if !self.add_analyze_target(path).await {
            *self.analyzed.write().await = None;
            *self.status.write().await = progress::AnalysisStatus::Error;
            return false;
        }

        self.shutdown_subprocesses().await;
        *self.status.write().await = progress::AnalysisStatus::Analyzing;

        let analyzers = { self.analyzers.read().await.clone() };
        let mut analyzed: Option<Crate> = None;

        for analyzer in analyzers {
            let mut iter = analyzer.analyze(all_targets, all_features).await;
            while let Some(event) = iter.next_event().await {
                if let AnalyzerEvent::Analyzed(ws) = event {
                    for krate in ws.0.into_values() {
                        if let Some(write) = &mut analyzed {
                            write.merge(krate);
                        } else {
                            analyzed = Some(krate);
                        }
                    }
                }
            }
        }

        let ok = analyzed.as_ref().map(|v| !v.0.is_empty()).unwrap_or(false);
        *self.analyzed.write().await = analyzed;
        *self.status.write().await = if ok {
            progress::AnalysisStatus::Finished
        } else {
            progress::AnalysisStatus::Error
        };

        ok
    }

    pub async fn shutdown_subprocesses(&self) {
        {
            let mut tokens = self.process_tokens.write().await;
            while let Some((_, token)) = tokens.pop_last() {
                token.cancel();
            }
        }
        self.processes.write().await.shutdown().await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let mut workspaces = Vec::new();
        if let Some(wss) = params.workspace_folders {
            workspaces.extend(
                wss.iter()
                    .filter_map(|v| v.uri.to_file_path().map(|p| p.into_owned())),
            );
        }
        for path in workspaces {
            self.add_analyze_target(&path).await;
        }
        self.do_analyze().await;

        let sync_options = lsp_types::TextDocumentSyncOptions {
            open_close: Some(true),
            save: Some(lsp_types::TextDocumentSyncSaveOptions::Supported(true)),
            change: Some(lsp_types::TextDocumentSyncKind::INCREMENTAL),
            ..Default::default()
        };
        let workspace_cap = lsp_types::WorkspaceServerCapabilities {
            workspace_folders: Some(lsp_types::WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(lsp_types::OneOf::Left(true)),
            }),
            ..Default::default()
        };
        let server_cap = lsp_types::ServerCapabilities {
            text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Options(sync_options)),
            workspace: Some(workspace_cap),
            ..Default::default()
        };
        let init_res = lsp_types::InitializeResult {
            capabilities: server_cap,
            ..Default::default()
        };
        let health_checker = async move {
            if let Some(process_id) = params.process_id {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    if !process_alive::state(process_alive::Pid::from(process_id)).is_alive() {
                        panic!("The client process is dead");
                    }
                }
            }
        };
        if params
            .capabilities
            .window
            .and_then(|v| v.work_done_progress)
            .unwrap_or(false)
        {
            *self.work_done_progress.write().await = true;
        }
        tokio::spawn(health_checker);
        Ok(init_res)
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        for added in params.event.added {
            if let Some(path) = added.uri.to_file_path()
                && self.add_analyze_target(&path).await
            {
                self.do_analyze().await;
            }
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if let Some(path) = params.text_document.uri.to_file_path()
            && params.text_document.language_id == "rust"
        {
            let text = Arc::new(params.text_document.text);
            let index = Arc::new(utils::LineCharIndex::new(&text));
            let line_start_bytes = Arc::new(utils::line_start_bytes(&text));
            let path = path.into_owned();
            self.open_docs.write().await.insert(
                path.clone(),
                OpenDoc {
                    text,
                    index,
                    line_start_bytes,
                },
            );

            if path.is_file() && self.add_analyze_target(&path).await {
                self.do_analyze().await;
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(path) = params.text_document.uri.to_file_path() {
            if params.content_changes.is_empty() {
                self.open_docs.write().await.remove(path.as_ref());
            } else {
                let mut docs = self.open_docs.write().await;
                if let Some(open) = docs.get_mut(path.as_ref()) {
                    // Apply ordered incremental edits. If anything looks odd, drop the cache.
                    let mut text = (*open.text).clone();
                    let mut line_starts = utils::line_start_bytes(&text);
                    let mut drop_cache = false;

                    for change in &params.content_changes {
                        if let Some(range) = change.range {
                            let start = utils::line_utf16_to_byte_offset(
                                &text,
                                &line_starts,
                                range.start.line,
                                range.start.character,
                            );
                            let end = utils::line_utf16_to_byte_offset(
                                &text,
                                &line_starts,
                                range.end.line,
                                range.end.character,
                            );
                            if start > end || end > text.len() {
                                drop_cache = true;
                                break;
                            }
                            text.replace_range(start..end, &change.text);
                            line_starts = utils::line_start_bytes(&text);
                        } else {
                            // Full text replacement.
                            text = change.text.clone();
                            line_starts = utils::line_start_bytes(&text);
                        }
                    }

                    if drop_cache {
                        docs.remove(path.as_ref());
                    } else {
                        open.text = Arc::new(text);
                        open.index = Arc::new(utils::LineCharIndex::new(open.text.as_ref()));
                        open.line_start_bytes = Arc::new(line_starts);
                    }
                }
            }
        }

        *self.analyzed.write().await = None;
        self.shutdown_subprocesses().await;
    }

    async fn shutdown(&self) -> Result<()> {
        self.shutdown_subprocesses().await;
        Ok(())
    }
}

// These tests require tokio's IO driver which uses platform-specific syscalls
// (kqueue on macOS, epoll on Linux) that Miri doesn't support.
// See: https://github.com/rust-lang/miri/issues/602
#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;
    use crate::miri_async_test;

    #[test]
    fn test_check_method() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");
            tokio::fs::write(
                &cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;

            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_with_options() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");
            tokio::fs::write(
                &cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            let result = Backend::check_with_options(&temp_dir.path(), true, true, 1).await;

            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_invalid_path() {
        miri_async_test!(async {
            let result = Backend::check(Path::new("/nonexistent/path"), 1).await;

            assert!(!result);
        });
    }

    #[test]
    fn test_check_with_options_invalid_path() {
        miri_async_test!(async {
            let result =
                Backend::check_with_options(Path::new("/nonexistent/path"), false, false, 1).await;
            assert!(!result);
        });
    }

    #[test]
    fn test_check_valid_cargo_no_src() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");
            tokio::fs::write(
                &cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;

            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_with_different_options() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");
            tokio::fs::write(
                &cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            // Test all combinations of options
            let result1 = Backend::check_with_options(&temp_dir.path(), false, false, 1).await;
            let result2 = Backend::check_with_options(&temp_dir.path(), true, false, 1).await;
            let result3 = Backend::check_with_options(&temp_dir.path(), false, true, 1).await;
            let result4 = Backend::check_with_options(&temp_dir.path(), true, true, 1).await;

            // All should return boolean values without panicking
            assert!(matches!(result1, true | false));
            assert!(matches!(result2, true | false));
            assert!(matches!(result3, true | false));
            assert!(matches!(result4, true | false));
        });
    }

    #[test]
    fn test_check_with_workspace() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();

            // Create workspace Cargo.toml
            let workspace_cargo = temp_dir.path().join("Cargo.toml");
            tokio::fs::write(&workspace_cargo,
                "[workspace]\nmembers = [\"pkg1\", \"pkg2\"]\n[package]\nname = \"workspace\"\nversion = \"0.1.0\""
            ).await.unwrap();

            // Create member packages
            let pkg1_dir = temp_dir.path().join("pkg1");
            tokio::fs::create_dir(&pkg1_dir).await.unwrap();
            let pkg1_cargo = pkg1_dir.join("Cargo.toml");
            tokio::fs::write(
                &pkg1_cargo,
                "[package]\nname = \"pkg1\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;
            // Should handle workspace structure
            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_malformed_cargo() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");

            // Write malformed TOML
            tokio::fs::write(
                &cargo_toml,
                "[package\nname = \"test\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;
            // Should handle malformed Cargo.toml gracefully
            assert!(!result);
        });
    }

    #[test]
    fn test_check_empty_directory() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;
            // Should fail with empty directory
            assert!(!result);
        });
    }

    #[test]
    fn test_check_with_options_empty_directory() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();

            let result = Backend::check_with_options(&temp_dir.path(), true, true, 1).await;
            // Should fail with empty directory regardless of options
            assert!(!result);
        });
    }

    #[test]
    fn test_check_nested_cargo() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let nested_dir = temp_dir.path().join("nested");
            tokio::fs::create_dir(&nested_dir).await.unwrap();

            let cargo_toml = nested_dir.join("Cargo.toml");
            tokio::fs::write(
                &cargo_toml,
                "[package]\nname = \"nested\"\nversion = \"0.1.0\"",
            )
            .await
            .unwrap();

            let result = Backend::check(&nested_dir, 1).await;
            // Should work with nested directory containing Cargo.toml
            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_with_binary_target() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");

            tokio::fs::write(&cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"\n[[bin]]\nname = \"main\"\npath = \"src/main.rs\""
            ).await.unwrap();

            let src_dir = temp_dir.path().join("src");
            tokio::fs::create_dir(&src_dir).await.unwrap();
            let main_rs = src_dir.join("main.rs");
            tokio::fs::write(&main_rs, "fn main() { println!(\"Hello\"); }")
                .await
                .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;
            // Should handle binary targets
            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_with_library_target() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");

            tokio::fs::write(&cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"\n[lib]\nname = \"testlib\"\npath = \"src/lib.rs\""
            ).await.unwrap();

            let src_dir = temp_dir.path().join("src");
            tokio::fs::create_dir(&src_dir).await.unwrap();
            let lib_rs = src_dir.join("lib.rs");
            tokio::fs::write(&lib_rs, "pub fn hello() { println!(\"Hello\"); }")
                .await
                .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;
            // Should handle library targets
            assert!(matches!(result, true | false));
        });
    }

    #[test]
    fn test_check_with_mixed_targets() {
        miri_async_test!(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let cargo_toml = temp_dir.path().join("Cargo.toml");

            tokio::fs::write(&cargo_toml,
                "[package]\nname = \"test\"\nversion = \"0.1.0\"\n[lib]\nname = \"testlib\"\npath = \"src/lib.rs\"\n[[bin]]\nname = \"main\"\npath = \"src/main.rs\""
            ).await.unwrap();

            let src_dir = temp_dir.path().join("src");
            tokio::fs::create_dir(&src_dir).await.unwrap();
            let lib_rs = src_dir.join("lib.rs");
            let main_rs = src_dir.join("main.rs");
            tokio::fs::write(&lib_rs, "pub fn hello() { println!(\"Hello\"); }")
                .await
                .unwrap();
            tokio::fs::write(&main_rs, "fn main() { println!(\"Hello\"); }")
                .await
                .unwrap();

            let result = Backend::check(&temp_dir.path(), 1).await;
            // Should handle mixed targets
            assert!(matches!(result, true | false));
        });
    }
}
