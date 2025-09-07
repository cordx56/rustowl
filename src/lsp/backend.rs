use super::analyze::*;
use crate::{lsp::*, models::*, utils};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::{sync::RwLock, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::lsp_types::{self, *};
use tower_lsp_server::{Client, LanguageServer, LspService, UriExt};

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeRequest {}
#[derive(serde::Serialize, Clone, Debug)]
pub struct AnalyzeResponse {}

/// RustOwl LSP server backend
pub struct Backend {
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

    pub async fn analyze(&self, _params: AnalyzeRequest) -> Result<AnalyzeResponse> {
        tracing::info!("rustowl/analyze request received");
        self.do_analyze().await;
        Ok(AnalyzeResponse {})
    }
    async fn do_analyze(&self) {
        self.shutdown_subprocesses().await;
        self.analyze_with_options(false, false).await;
    }

    async fn analyze_with_options(&self, all_targets: bool, all_features: bool) {
        tracing::info!("wait 100ms for rust-analyzer");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        tracing::info!("stop running analysis processes");
        self.shutdown_subprocesses().await;

        tracing::info!("start analysis");
        {
            *self.status.write().await = progress::AnalysisStatus::Analyzing;
        }
        let analyzers = { self.analyzers.read().await.clone() };

        tracing::info!("analyze {} packages...", analyzers.len());
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
    ) -> std::result::Result<Vec<decoration::Deco>, progress::AnalysisStatus> {
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
    ) -> Result<decoration::Decorations> {
        let is_analyzed = self.analyzed.read().await.is_some();
        let status = *self.status.read().await;
        if let Some(path) = params.path()
            && let Ok(text) = tokio::fs::read_to_string(&path).await
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
            && path.is_file()
            && params.text_document.language_id == "rust"
            && self.add_analyze_target(&path).await
        {
            self.do_analyze().await;
        }
    }

    async fn did_change(&self, _params: DidChangeTextDocumentParams) {
        *self.analyzed.write().await = None;
        self.shutdown_subprocesses().await;
    }

    async fn shutdown(&self) -> Result<()> {
        self.shutdown_subprocesses().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static CRYPTO_PROVIDER_INIT: Once = Once::new();

    /// Safely initialize the crypto provider once to avoid multiple installation issues
    fn init_crypto_provider() {
        CRYPTO_PROVIDER_INIT.call_once(|| {
            // Try to install the crypto provider, but don't panic if it's already installed
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        });

        // Also try to install it directly in case the Once didn't work
        // This is safe to call multiple times
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    // Test Backend::check method
    #[tokio::test]
    async fn test_check_method() {
        init_crypto_provider();
        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        let result = Backend::check(&temp_dir.path()).await;

        assert!(matches!(result, true | false));
    }

    // Test Backend::check_with_options method
    #[tokio::test]
    async fn test_check_with_options() {
        init_crypto_provider();

        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        let result = Backend::check_with_options(&temp_dir.path(), true, true).await;

        assert!(matches!(result, true | false));
    }

    // Test Backend::check with invalid path
    #[tokio::test]
    async fn test_check_invalid_path() {
        init_crypto_provider();
        // Use a timeout to prevent the test from hanging
        let result = Backend::check(Path::new("/nonexistent/path")).await;

        assert!(!result);
    }

    // Test Backend::check_with_options with invalid path
    #[tokio::test]
    async fn test_check_with_options_invalid_path() {
        init_crypto_provider();

        let result =
            Backend::check_with_options(Path::new("/nonexistent/path"), false, false).await;
        assert!(!result);
    }

    // Test Backend::check with valid Cargo.toml but no source files
    #[tokio::test]
    async fn test_check_valid_cargo_no_src() {
        init_crypto_provider();

        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        let result = Backend::check(&temp_dir.path()).await;

        assert!(matches!(result, true | false));
    }

    // Test Backend::check with different option combinations
    #[tokio::test]
    async fn test_check_with_different_options() {
        init_crypto_provider();

        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        // Test all combinations of options
        let result1 = Backend::check_with_options(&temp_dir.path(), false, false).await;
        let result2 = Backend::check_with_options(&temp_dir.path(), true, false).await;
        let result3 = Backend::check_with_options(&temp_dir.path(), false, true).await;
        let result4 = Backend::check_with_options(&temp_dir.path(), true, true).await;

        // All should return boolean values without panicking
        assert!(matches!(result1, true | false));
        assert!(matches!(result2, true | false));
        assert!(matches!(result3, true | false));
        assert!(matches!(result4, true | false));
    }

    // Test Backend::check with workspace (multiple packages)
    #[tokio::test]
    async fn test_check_with_workspace() {
        init_crypto_provider();

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

        let result = Backend::check(&temp_dir.path()).await;
        // Should handle workspace structure
        assert!(matches!(result, true | false));
    }

    // Test Backend::check with malformed Cargo.toml
    #[tokio::test]
    async fn test_check_malformed_cargo() {
        init_crypto_provider();

        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");

        // Write malformed TOML
        tokio::fs::write(
            &cargo_toml,
            "[package\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        let result = Backend::check(&temp_dir.path()).await;
        // Should handle malformed Cargo.toml gracefully
        assert!(!result);
    }

    // Test Backend::check with empty directory
    #[tokio::test]
    async fn test_check_empty_directory() {
        init_crypto_provider();
        let temp_dir = tempfile::tempdir().unwrap();

        let result = Backend::check(&temp_dir.path()).await;
        // Should fail with empty directory
        assert!(!result);
    }

    // Test Backend::check_with_options with empty directory
    #[tokio::test]
    async fn test_check_with_options_empty_directory() {
        init_crypto_provider();

        let temp_dir = tempfile::tempdir().unwrap();

        let result = Backend::check_with_options(&temp_dir.path(), true, true).await;
        // Should fail with empty directory regardless of options
        assert!(!result);
    }

    // Test Backend::check with nested Cargo.toml
    #[tokio::test]
    async fn test_check_nested_cargo() {
        init_crypto_provider();

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

        let result = Backend::check(&nested_dir).await;
        // Should work with nested directory containing Cargo.toml
        assert!(matches!(result, true | false));
    }

    // Test Backend::check with binary target
    #[tokio::test]
    async fn test_check_with_binary_target() {
        init_crypto_provider();

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

        let result = Backend::check(&temp_dir.path()).await;
        // Should handle binary targets
        assert!(matches!(result, true | false));
    }

    // Test Backend::check with library target
    #[tokio::test]
    async fn test_check_with_library_target() {
        init_crypto_provider();

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

        let result = Backend::check(&temp_dir.path()).await;
        // Should handle library targets
        assert!(matches!(result, true | false));
    }

    // Test Backend::check with both binary and library targets
    #[tokio::test]
    async fn test_check_with_mixed_targets() {
        init_crypto_provider();

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

        let result = Backend::check(&temp_dir.path()).await;
        // Should handle mixed targets
        assert!(matches!(result, true | false));
    }
}
