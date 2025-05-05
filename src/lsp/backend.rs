use crate::{lsp::*, models::*, toolchain, utils};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process,
    sync::RwLock,
    task::JoinSet,
};
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types;
use tower_lsp::{Client, LanguageServer, LspService};

const RUSTC_DRIVER_DIR: Option<&str> = option_env!("RUSTC_DRIVER_DIR");

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum CargoCheckMessage {
    #[allow(unused)]
    CompilerArtifact {},
    #[allow(unused)]
    BuildFinished {},
}

type Subprocess = Option<u32>;

/// RustOwl LSP server backend
#[derive(Debug)]
pub struct Backend {
    #[allow(unused)]
    client: Client,
    workspaces: Arc<RwLock<Vec<PathBuf>>>,
    roots: Arc<RwLock<HashMap<PathBuf, PathBuf>>>,
    status: Arc<RwLock<progress::AnalysisStatus>>,
    analyzed: Arc<RwLock<Option<Workspace>>>,
    processes: Arc<RwLock<JoinSet<()>>>,
    subprocesses: Arc<RwLock<Vec<Subprocess>>>,
    work_done_progress: Arc<RwLock<bool>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            workspaces: Arc::new(RwLock::new(Vec::new())),
            roots: Arc::new(RwLock::new(HashMap::new())),
            analyzed: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(progress::AnalysisStatus::Finished)),
            processes: Arc::new(RwLock::new(JoinSet::new())),
            subprocesses: Arc::new(RwLock::new(vec![])),
            work_done_progress: Arc::new(RwLock::new(false)),
        }
    }
    /// returns `true` if the root was not registered
    async fn set_roots(&self, path: PathBuf) -> bool {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap().to_path_buf()
        };
        for w in &*self.workspaces.read().await {
            if dir.starts_with(w) {
                let mut write = self.roots.write().await;
                if let Ok(metadata) = cargo_metadata::MetadataCommand::new()
                    .current_dir(&dir)
                    .exec()
                {
                    let path = metadata.workspace_root;
                    if !write.contains_key(path.as_std_path()) {
                        log::info!("add {} to watch list", path);

                        let target = metadata
                            .target_directory
                            .as_std_path()
                            .to_path_buf()
                            .join("owl");
                        tokio::fs::create_dir_all(&target).await.unwrap();

                        write.insert(path.as_std_path().to_path_buf(), target);
                        return true;
                    }
                }
            }
        }
        false
    }
    async fn set_workspace(&self, ws: PathBuf) {
        self.workspaces.write().await.push(ws);
    }

    async fn abort_subprocess(&self) {
        #[cfg(unix)]
        while let Some(pid) = self.subprocesses.write().await.pop() {
            if let Some(pid) = pid {
                unsafe {
                    libc::killpg(pid.try_into().unwrap(), libc::SIGTERM);
                }
            }
        }
    }

    async fn analyze(&self) {
        log::info!("wait 100ms for rust-analyzer");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        log::info!("stop running analysis processes");
        let mut join = self.processes.write().await;
        join.shutdown().await;
        self.abort_subprocess().await;

        log::info!("start analysis");
        {
            *self.status.write().await = progress::AnalysisStatus::Analyzing;
        }
        let roots = { self.roots.read().await.clone() };

        for (root, target) in roots {
            // progress report
            let meta = cargo_metadata::MetadataCommand::new()
                .current_dir(&root)
                .exec()
                .ok();
            let dep_count = meta
                .as_ref()
                .and_then(|v| v.resolve.as_ref().map(|w| w.nodes.len()))
                .unwrap_or(0);

            let mut progress_token = None;
            let package_name = meta.and_then(|v| v.root_package().map(|w| w.name.clone()));
            if let Some(package_name) = &package_name {
                log::info!("clear cargo cache");
                let mut command = process::Command::new("cargo");
                command
                    .args(["clean", "--package", package_name])
                    .env("CARGO_TARGET_DIR", &target)
                    .current_dir(&root)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                command.spawn().unwrap().wait().await.ok();
            }

            let client = self.client.clone();
            if *self.work_done_progress.read().await {
                progress_token = Some(
                    progress::ProgressToken::begin(
                        client,
                        package_name.as_ref().map(|v| format!("analyzing {v}")),
                    )
                    .await,
                )
            };

            let sysroot = toolchain::get_sysroot().await;
            let mut command = if let Ok(cargo_path) = &env::var("CARGO") {
                log::info!("using toolchain cargo: {}", cargo_path);
                process::Command::new(cargo_path)
            } else {
                log::info!("using default cargo",);
                process::Command::new("cargo")
            };
            command
                .args([
                    "check",
                    "--all-targets",
                    "--all-features",
                    "--keep-going",
                    "--message-format=json",
                ])
                .env("CARGO_TARGET_DIR", &target)
                .env_remove("RUSTC_WRAPPER")
                .current_dir(&root)
                .stdout(std::process::Stdio::piped())
                .kill_on_drop(true);

            // set rustowlc & library path
            let rustowlc_path = {
                let under_sysroot = toolchain::get_runtime_dir().await.join("rustowlc");
                if under_sysroot.is_file() {
                    under_sysroot.to_string_lossy().to_string()
                } else {
                    "rustowlc".to_owned()
                }
            };
            command
                .env("RUSTC", &rustowlc_path)
                .env("RUSTC_WORKSPACE_WRAPPER", &rustowlc_path)
                .env("RUSTC_BOOTSTRAP", "1") // Support nightly projects
                .env(
                    "CARGO_ENCODED_RUSTFLAGS",
                    format!("--sysroot={}", sysroot.display()),
                );
            if let Some(driver_dir) = RUSTC_DRIVER_DIR {
                #[cfg(target_os = "linux")]
                {
                    let mut paths =
                        env::split_paths(&env::var("LD_LIBRARY_PATH").unwrap_or("".to_owned()))
                            .collect::<std::collections::VecDeque<_>>();
                    paths.push_front(sysroot.join(driver_dir));
                    let paths = env::join_paths(paths).unwrap();
                    command.env("LD_LIBRARY_PATH", paths);
                }
                #[cfg(target_os = "macos")]
                {
                    let mut paths = env::split_paths(
                        &env::var("DYLD_FALLBACK_LIBRARY_PATH").unwrap_or("".to_owned()),
                    )
                    .collect::<std::collections::VecDeque<_>>();
                    paths.push_front(sysroot.join(driver_dir));
                    let paths = env::join_paths(paths).unwrap();
                    command.env("DYLD_FALLBACK_LIBRARY_PATH", paths);
                }
                #[cfg(target_os = "windows")]
                {
                    let mut paths = env::split_paths(&env::var_os("Path").unwrap())
                        .collect::<std::collections::VecDeque<_>>();
                    paths.push_front(sysroot.join(driver_dir));
                    let paths = env::join_paths(paths).unwrap();
                    command.env("Path", paths);
                }
            }

            #[cfg(unix)]
            unsafe {
                command.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
            if log::max_level().to_level().is_none() {
                command.stderr(std::process::Stdio::null());
            }
            log::info!("start checking {}", root.display());
            let mut child = command.spawn().unwrap();
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
            let analyzed = self.analyzed.clone();
            join.spawn(async move {
                let mut build_count = 0;
                while let Ok(Some(line)) = stdout.next_line().await {
                    if let Ok(CargoCheckMessage::CompilerArtifact { .. }) =
                        serde_json::from_str(&line)
                    {
                        build_count += 1;
                        log::info!("{build_count} crates checked");
                        if let Some(token) = &progress_token {
                            let percentage = (build_count * 100 / dep_count).min(100);
                            token
                                .report(
                                    package_name.as_ref().map(|v| format!("analyzing {v}")),
                                    Some(percentage as u32),
                                )
                                .await;
                        }
                    }
                    if let Ok(ws) = serde_json::from_str::<Workspace>(&line) {
                        let write = &mut *analyzed.write().await;
                        if let Some(write) = write {
                            write.merge(ws);
                        } else {
                            *write = Some(ws);
                        }
                    }
                }
                if let Some(progress_token) = progress_token {
                    progress_token.finish().await;
                }
            });

            let pid = child.id();
            let subprocesses = self.subprocesses.clone();
            let cache_target = target.join("cache.json");
            let analyzed = self.analyzed.clone();
            let status = self.status.clone();
            join.spawn(async move {
                let _ = child.wait().await;
                log::info!("check finished");
                let analyzed = &*analyzed.read().await;
                let mut write = subprocesses.write().await;
                *write = write.iter().filter(|v| **v != pid).copied().collect();
                if write.is_empty() {
                    let mut status = status.write().await;
                    if *status != progress::AnalysisStatus::Error {
                        if analyzed.as_ref().map(|v| v.0.len()).unwrap_or(0) == 0 {
                            *status = progress::AnalysisStatus::Error;
                        } else {
                            *status = progress::AnalysisStatus::Finished;
                        }
                    }
                }

                if let Ok(mut cache_file) = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(cache_target)
                    .await
                {
                    cache_file
                        .write_all(&serde_json::to_vec(analyzed).unwrap())
                        .await
                        .ok();
                }
            });
            self.subprocesses.write().await.push(pid);
        }
    }

    async fn decos(
        &self,
        filepath: &Path,
        position: Loc,
    ) -> Result<Vec<decoration::Deco>, progress::AnalysisStatus> {
        let mut selected = decoration::SelectLocal::new(position);
        let mut error = progress::AnalysisStatus::Error;
        if let Some(analyzed) = &*self.analyzed.read().await {
            for (_crate_name, krate) in analyzed.0.iter() {
                for (filename, file) in krate.0.iter() {
                    if filepath == PathBuf::from(filename) {
                        if !file.items.is_empty() {
                            error = progress::AnalysisStatus::Finished;
                        }
                        for item in &file.items {
                            utils::mir_visit(item, &mut selected);
                        }
                    }
                }
            }

            let mut calc = decoration::CalcDecos::new(selected.selected().iter().copied());
            for (_crate_name, krate) in analyzed.0.iter() {
                for (filename, file) in krate.0.iter() {
                    if filepath == PathBuf::from(filename) {
                        for item in &file.items {
                            utils::mir_visit(item, &mut calc);
                        }
                    }
                }
            }
            calc.handle_overlapping();
            let decos = calc.decorations();
            if !decos.is_empty() {
                Ok(decos)
            } else {
                Err(error)
            }
        } else {
            Err(error)
        }
    }

    pub async fn cursor(
        &self,
        params: decoration::CursorRequest,
    ) -> jsonrpc::Result<decoration::Decorations> {
        let is_analyzed = self.analyzed.read().await.is_some();
        let status = *self.status.read().await;
        if let Some(path) = params.path() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let position = params.position();
                let pos = Loc(utils::line_char_to_index(
                    &text,
                    position.line,
                    position.character,
                ));
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
                let decorations = decos.into_iter().map(|v| v.to_lsp_range(&text)).collect();
                return Ok(decoration::Decorations {
                    is_analyzed,
                    status,
                    path: Some(path),
                    decorations,
                });
            }
        }
        Ok(decoration::Decorations {
            is_analyzed,
            status,
            path: None,
            decorations: Vec::new(),
        })
    }

    pub async fn check(path: PathBuf) -> bool {
        let (service, _) = LspService::build(Backend::new).finish();
        let backend = service.inner();
        backend.set_workspace(path.clone()).await;
        backend.set_roots(path).await;
        backend.analyze().await;
        while backend.processes.write().await.join_next().await.is_some() {}
        backend
            .analyzed
            .read()
            .await
            .as_ref()
            .map(|v| !v.0.is_empty())
            .unwrap_or(false)
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                if let Err(err) = self.shutdown().await {
                    log::error!("failed to shutdown the server gracefully: {err}");
                };
            });
        });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: lsp_types::InitializeParams,
    ) -> jsonrpc::Result<lsp_types::InitializeResult> {
        if let Some(wss) = params.workspace_folders {
            for ws in wss {
                if let Ok(path) = ws.uri.to_file_path() {
                    self.set_workspace(path).await;
                }
            }
        }
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

    async fn did_change_workspace_folders(
        &self,
        params: lsp_types::DidChangeWorkspaceFoldersParams,
    ) -> () {
        for added in params.event.added {
            if let Ok(path) = added.uri.to_file_path() {
                self.set_workspace(path).await;
            }
        }
        self.analyze().await;
    }
    async fn did_open(&self, params: lsp_types::DidOpenTextDocumentParams) {
        if params.text_document.language_id == "rust"
            && self
                .set_roots(params.text_document.uri.to_file_path().unwrap())
                .await
        {
            self.analyze().await;
        }
    }

    async fn did_save(&self, _params: lsp_types::DidSaveTextDocumentParams) {
        self.analyze().await;
    }
    async fn did_change(&self, _params: lsp_types::DidChangeTextDocumentParams) {
        *self.analyzed.write().await = None;
        self.processes.write().await.shutdown().await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.processes.write().await.shutdown().await;
        self.abort_subprocess().await;
        Ok(())
    }
}
