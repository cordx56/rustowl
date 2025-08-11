use super::analyze::*;
use crate::{lsp::*, models::*, utils};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::{sync::RwLock, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types;
use tower_lsp::{Client, LanguageServer, LspService};

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeRequest {}
#[derive(serde::Serialize, Clone, Debug)]
pub struct AnalyzeResponse {}

/// RustOwl LSP server backend
pub struct Backend {
    #[allow(unused)]
    client: Client,
    analyzers: Arc<RwLock<Vec<Analyzer>>>,
    status: Arc<RwLock<progress::AnalysisStatus>>,
    analyzed: Arc<RwLock<Option<Crate>>>,
    processes: Arc<RwLock<JoinSet<()>>>,
    process_tokens: Arc<RwLock<BTreeMap<usize, CancellationToken>>>,
    work_done_progress: Arc<RwLock<bool>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            analyzers: Arc::new(RwLock::new(Vec::new())),
            analyzed: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(progress::AnalysisStatus::Finished)),
            processes: Arc::new(RwLock::new(JoinSet::new())),
            process_tokens: Arc::new(RwLock::new(BTreeMap::new())),
            work_done_progress: Arc::new(RwLock::new(false)),
        }
    }

    async fn add_analyze_target(&self, path: &Path) -> bool {
        if let Ok(new_analyzer) = Analyzer::new(&path).await {
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

    pub async fn analyze(&self, _params: AnalyzeRequest) -> jsonrpc::Result<AnalyzeResponse> {
        log::info!("rustowl/analyze request received");
        self.do_analyze().await;
        Ok(AnalyzeResponse {})
    }
    async fn do_analyze(&self) {
        self.analyze_with_options(false, false).await;
    }

    async fn analyze_with_options(&self, all_targets: bool, all_features: bool) {
        log::info!("wait 100ms for rust-analyzer");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        log::info!("stop running analysis processes");
        self.shutdown_subprocesses().await;

        log::info!("start analysis");
        {
            *self.status.write().await = progress::AnalysisStatus::Analyzing;
        }
        let analyzers = { self.analyzers.read().await.clone() };

        log::info!("analyze {} packages...", analyzers.len());
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
                let mut analyzed_package_count = 0;
                while let Some(event) = tokio::select! {
                    _ = cancellation_token.cancelled() => None,
                    event = iter.next_event() => event,
                } {
                    match event {
                        AnalyzerEvent::CrateChecked {
                            package,
                            package_count,
                        } => {
                            analyzed_package_count += 1;
                            if let Some(token) = &progress_token {
                                let percentage =
                                    (analyzed_package_count * 100 / package_count).min(100);
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
    ) -> Result<Vec<decoration::Deco>, progress::AnalysisStatus> {
        let mut selected = decoration::SelectLocal::new(position);
        let mut error = progress::AnalysisStatus::Error;
        if let Some(analyzed) = &*self.analyzed.read().await {
            for (filename, file) in analyzed.0.iter() {
                if filepath == PathBuf::from(filename) {
                    if !file.items.is_empty() {
                        error = progress::AnalysisStatus::Finished;
                    }
                    for item in &file.items {
                        utils::mir_visit(item, &mut selected);
                    }
                }
            }

            let mut calc = decoration::CalcDecos::new(selected.selected().iter().copied());
            for (filename, file) in analyzed.0.iter() {
                if filepath == PathBuf::from(filename) {
                    for item in &file.items {
                        utils::mir_visit(item, &mut calc);
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
        if let Some(path) = params.path()
            && let Ok(text) = std::fs::read_to_string(&path)
        {
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
        Ok(decoration::Decorations {
            is_analyzed,
            status,
            path: None,
            decorations: Vec::new(),
        })
    }

    pub async fn check(path: impl AsRef<Path>) -> bool {
        Self::check_with_options(path, false, false).await
    }

    pub async fn check_with_options(
        path: impl AsRef<Path>,
        all_targets: bool,
        all_features: bool,
    ) -> bool {
        let path = path.as_ref();
        let (service, _) = LspService::build(Backend::new).finish();
        let backend = service.inner();

        if backend.add_analyze_target(path).await {
            backend
                .analyze_with_options(all_targets, all_features)
                .await;
            while backend.processes.write().await.join_next().await.is_some() {}
            backend
                .analyzed
                .read()
                .await
                .as_ref()
                .map(|v| !v.0.is_empty())
                .unwrap_or(false)
        } else {
            false
        }
    }

    pub async fn shutdown_subprocesses(&self) {
        let mut tokens = self.process_tokens.write().await;
        while let Some((_, token)) = tokens.pop_last() {
            token.cancel();
        }
        self.processes.write().await.shutdown().await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: lsp_types::InitializeParams,
    ) -> jsonrpc::Result<lsp_types::InitializeResult> {
        let mut workspaces = Vec::new();
        if let Some(root) = params.root_uri
            && let Ok(path) = root.to_file_path()
        {
            workspaces.push(path);
        }
        if let Some(wss) = params.workspace_folders {
            workspaces.extend(wss.iter().filter_map(|v| v.uri.to_file_path().ok()));
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

    async fn did_change_workspace_folders(
        &self,
        params: lsp_types::DidChangeWorkspaceFoldersParams,
    ) -> () {
        for added in params.event.added {
            if let Ok(path) = added.uri.to_file_path()
                && self.add_analyze_target(&path).await
            {
                self.do_analyze().await;
            }
        }
    }

    async fn did_open(&self, params: lsp_types::DidOpenTextDocumentParams) {
        if let Ok(path) = params.text_document.uri.to_file_path()
            && path.is_file()
            && params.text_document.language_id == "rust"
            && self.add_analyze_target(&path).await
        {
            self.do_analyze().await;
        }
    }

    async fn did_change(&self, _params: lsp_types::DidChangeTextDocumentParams) {
        *self.analyzed.write().await = None;
        self.shutdown_subprocesses().await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.shutdown_subprocesses().await;
        Ok(())
    }
}
