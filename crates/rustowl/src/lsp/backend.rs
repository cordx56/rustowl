use super::analyze::*;
use super::cancel::Cancel;
use super::results::Results;
use crate::{lsp::*, models::*, utils};
use arc_swap::ArcSwap;
use archery::{ArcK, SharedPointer};
use rpds::HashTrieMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::Notify;
use tower_lsp_server::gen_lsp_types;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::{Client, LanguageServer, LspService};

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeRequest {}
#[derive(serde::Serialize, Clone, Debug)]
pub struct AnalyzeResponse {}

/// An immutable view of everything analysed so far.
///
/// Published behind an `Arc`: readers traverse it without a lock, and a
/// superseding view never mutates what a reader is looking at.
#[derive(Debug)]
pub struct Snapshot {
    /// Per workspace root, the files analysed from it. Keying by root keeps
    /// workspaces from colliding on shared paths.
    pub crates: Workspace,
}

impl Snapshot {
    pub(crate) fn empty() -> Self {
        Self {
            crates: Workspace(Default::default()),
        }
    }

    /// A snapshot with one analysed workspace folded in under its root.
    pub(crate) fn merged(&self, target: &str, ws: &Workspace) -> Option<Snapshot> {
        let mut merged: Option<SharedPointer<Crate, ArcK>> = None;
        for (_, krate) in ws.0.iter() {
            merged = Some(match merged {
                Some(current) => match current.as_ref().merged(krate.as_ref()) {
                    Some(merged) => SharedPointer::new(merged),
                    None => current,
                },
                None => SharedPointer::clone(krate),
            });
        }
        let key: Arc<str> = Arc::from(target);
        let merged = merged?;
        let merged = match self.crates.0.get(&key) {
            Some(existing) => match existing.as_ref().merged(merged.as_ref()) {
                Some(merged) => SharedPointer::new(merged),
                None => SharedPointer::clone(existing),
            },
            None => merged,
        };
        // The crates that were not delivered keep their allocation.
        let mut crates: HashTrieMap<Arc<str>, SharedPointer<Crate, ArcK>, ArcK> =
            HashTrieMap::default();
        for (name, krate) in self.crates.0.iter() {
            if name.as_ref() == target {
                continue;
            }
            crates = crates.insert(Arc::clone(name), SharedPointer::clone(krate));
        }
        Some(Snapshot {
            crates: Workspace(crates.insert(key, merged)),
        })
    }

    pub(crate) fn without_file(&self, path: &str) -> Snapshot {
        let mut crates: Option<HashTrieMap<Arc<str>, SharedPointer<Crate, ArcK>, ArcK>> = None;
        for (name, krate) in self.crates.0.iter() {
            let updated = match krate.as_ref().without(path) {
                Some(updated) => SharedPointer::new(updated),
                None => SharedPointer::clone(krate),
            };
            crates = Some(match crates {
                Some(crates) => crates.insert(Arc::clone(name), updated),
                None => self.crates.0.insert(Arc::clone(name), updated),
            });
        }
        Snapshot {
            crates: Workspace(crates.unwrap_or_default()),
        }
    }

    /// Every file registered under `filepath`, across the workspaces that
    /// hold it.
    pub(crate) fn files_of(&self, filepath: &Path) -> Vec<&File> {
        self.crates
            .0
            .values()
            .filter_map(|krate| krate.file(&filepath.to_string_lossy()))
            .collect()
    }

    pub(crate) fn has_files(&self) -> bool {
        self.crates.0.values().any(|krate| !krate.0.is_empty())
    }
}

/// The analysis tasks that are running.
///
/// The count is atomic and the wait is a notification, never a lock. A task
/// that panics still gives its count back.
pub struct ProcessRegistry {
    inflight: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

struct CountGuard {
    inflight: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl Drop for CountGuard {
    fn drop(&mut self) {
        if self.inflight.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.notify.notify_waiters();
        }
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            inflight: Arc::new(AtomicUsize::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Run a task, counted until it finishes.
    pub fn spawn<F>(&self, task: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.inflight.fetch_add(1, Ordering::AcqRel);
        let guard = CountGuard {
            inflight: Arc::clone(&self.inflight),
            notify: Arc::clone(&self.notify),
        };
        tokio::spawn(async move {
            let _guard = guard;
            task.await;
        });
    }

    pub async fn drain(&self) {
        loop {
            // The wait is registered before the count is read: a task that
            // finishes in between must not be missed.
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.inflight.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

/// RustOwl LSP server backend
pub struct Backend {
    client: Client,
    analyzers: ArcSwap<rpds::Vector<SharedPointer<Analyzer, ArcK>, ArcK>>,
    status: Arc<ArcSwap<progress::AnalysisStatus>>,
    analyzed: Arc<Results>,
    processes: Arc<ProcessRegistry>,
    process_tokens: Arc<ArcSwap<rpds::HashTrieMap<usize, Cancel, ArcK>>>,
    /// the key each running task gets, so it can drop its own token
    next_token_key: AtomicUsize,
    work_done_progress: Arc<AtomicBool>,
    /// paths the editor currently has open; the compiler analyses only these
    open_files: ArcSwap<rpds::HashTrieSet<PathBuf, ArcK>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            analyzers: ArcSwap::from_pointee(rpds::Vector::default()),
            analyzed: Arc::new(Results::new()),
            status: Arc::new(ArcSwap::from_pointee(progress::AnalysisStatus::Finished)),
            processes: Arc::new(ProcessRegistry::new()),
            process_tokens: Arc::new(ArcSwap::from_pointee(rpds::HashTrieMap::default())),
            next_token_key: AtomicUsize::new(0),
            work_done_progress: Arc::new(AtomicBool::new(false)),
            open_files: ArcSwap::from_pointee(rpds::HashTrieSet::default()),
        }
    }

    async fn add_analyze_target(&self, path: &Path) -> bool {
        if let Ok(new_analyzer) = Analyzer::new(&path).await {
            let target = new_analyzer.target_path().to_path_buf();
            let mut analyzers = self.analyzers.load().as_ref().clone();
            for analyzer in &analyzers {
                if analyzer.target_path() == target {
                    return true;
                }
            }
            analyzers = analyzers.push_back(SharedPointer::new(new_analyzer));
            self.analyzers.store(Arc::new(analyzers));
            true
        } else {
            false
        }
    }

    pub async fn analyze(&self, _params: AnalyzeRequest) -> jsonrpc::Result<AnalyzeResponse> {
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
        let generation = self.analyzed.start_generation();
        self.status
            .store(Arc::new(progress::AnalysisStatus::Analyzing));
        let analyzers = self.analyzers.load().as_ref().clone();

        tracing::info!("analyze {} workspace(s)...", analyzers.len());
        for analyzer in analyzers.iter() {
            let analyzed = self.analyzed.clone();
            let client = self.client.clone();
            let work_done_progress = Arc::clone(&self.work_done_progress);
            let cancellation_token = Cancel::new();

            let cancellation_token_key = self.next_token_key.fetch_add(1, Ordering::AcqRel) + 1;
            let mut tokens = self.process_tokens.load().as_ref().clone();
            tokens = tokens.insert(cancellation_token_key, cancellation_token.clone());
            self.process_tokens.store(Arc::new(tokens));

            let process_tokens = Arc::clone(&self.process_tokens);
            let open_files: Vec<PathBuf> = self.open_files.load().iter().cloned().collect();
            let target_path = analyzer.target_path().to_string_lossy().into_owned();
            let analyzer = SharedPointer::clone(analyzer);
            self.processes.spawn(async move {
                let mut progress_token = None;
                if work_done_progress.load(Ordering::Relaxed) {
                    progress_token =
                        Some(progress::ProgressToken::begin(client).await)
                };

                let Some(mut iter) = analyzer
                    .analyze(all_targets, all_features, &open_files)
                    .await
                else {
                    tracing::error!(
                        "could not launch rustowl for {}",
                        analyzer.target_path().display()
                    );
                    return;
                };
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
                                    .report(format!("{package} analyzed"), percentage as u32)
                                    .await;
                            }
                        }
                        AnalyzerEvent::Analyzed(ws) => {
                            analyzed.accept(generation, &target_path, &ws);
                        }
                    }
                }
                // remove cancellation token from list
                let mut tokens = process_tokens.load().as_ref().clone();
                tokens = tokens.remove(&cancellation_token_key);
                process_tokens.store(Arc::new(tokens));

                if let Some(progress_token) = progress_token {
                    progress_token.finish().await;
                }
            });
        }

        let processes = self.processes.clone();
        let status = Arc::clone(&self.status);
        let analyzed = self.analyzed.clone();
        tokio::spawn(async move {
            processes.drain().await;
            let current = **status.load();
            if current != progress::AnalysisStatus::Error {
                if analyzed.snapshot().has_files() {
                    status.store(Arc::new(progress::AnalysisStatus::Finished));
                } else {
                    status.store(Arc::new(progress::AnalysisStatus::Error));
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
        let snapshot = self.analyzed.snapshot();
        for file in snapshot.files_of(filepath) {
            if !file.items.is_empty() {
                error = progress::AnalysisStatus::Finished;
            }
            for item in &file.items {
                utils::mir_visit(item, &mut selected);
            }
        }

        let mut calc = decoration::CalcDecos::new(selected.selected().iter().copied());
        for file in snapshot.files_of(filepath) {
            for item in &file.items {
                utils::mir_visit(item, &mut calc);
            }
        }
        calc.handle_overlapping();
        let decos = calc.decorations();
        if !decos.is_empty() {
            Ok(decos)
        } else {
            Err(error)
        }
    }

    pub async fn cursor(
        &self,
        params: decoration::CursorRequest,
    ) -> jsonrpc::Result<decoration::Decorations> {
        let is_analyzed = self.analyzed.snapshot().has_files();
        let status = **self.status.load();
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
            backend.processes.drain().await;
            backend.analyzed.snapshot().has_files()
        } else {
            false
        }
    }

    pub async fn shutdown_subprocesses(&self) {
        let tokens = self.process_tokens.load().as_ref().clone();
        for token in tokens.values() {
            token.cancel();
        }
        self.processes.drain().await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: gen_lsp_types::InitializeParams,
    ) -> jsonrpc::Result<gen_lsp_types::InitializeResult> {
        let mut workspaces = Vec::new();
        if let Some(gen_lsp_types::WorkspaceFolders::WorkspaceFolderList(folders)) =
            params.workspace_folders_initialize_params.workspace_folders
        {
            workspaces.extend(
                folders
                    .iter()
                    .filter_map(|v| utils::uri_to_file_path(&v.uri)),
            );
        }
        for path in workspaces {
            self.add_analyze_target(&path).await;
        }
        self.do_analyze().await;

        let sync_options = gen_lsp_types::TextDocumentSyncOptions {
            open_close: Some(true),
            save: Some(gen_lsp_types::Save::Bool(true)),
            change: Some(gen_lsp_types::TextDocumentSyncKind::Incremental),
            ..Default::default()
        };
        let workspace_cap = gen_lsp_types::WorkspaceOptions {
            workspace_folders: Some(gen_lsp_types::WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(gen_lsp_types::ChangeNotifications::Bool(true)),
            }),
            ..Default::default()
        };
        let server_cap = gen_lsp_types::ServerCapabilities {
            text_document_sync: Some(gen_lsp_types::TextDocumentSync::Options(sync_options)),
            workspace: Some(workspace_cap),
            ..Default::default()
        };
        let init_res = gen_lsp_types::InitializeResult {
            capabilities: server_cap,
            ..Default::default()
        };
        let health_checker = async move {
            // `processId` is an `i32` per the spec, while `process_alive` takes a
            // `u32` pid, so a nonsensical negative value just disables the check.
            if let Some(pid) = params.process_id.and_then(|v| u32::try_from(v).ok()) {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    if !process_alive::state(process_alive::Pid::from(pid)).is_alive() {
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
            self.work_done_progress.store(true, Ordering::Relaxed);
        }
        tokio::spawn(health_checker);
        Ok(init_res)
    }

    async fn did_change_workspace_folders(
        &self,
        params: gen_lsp_types::DidChangeWorkspaceFoldersParams,
    ) -> () {
        for added in params.event.added {
            if let Some(path) = utils::uri_to_file_path(&added.uri)
                && self.add_analyze_target(&path).await
            {
                self.do_analyze().await;
            }
        }
    }

    async fn did_open(&self, params: gen_lsp_types::DidOpenTextDocumentParams) {
        if let Some(path) = utils::uri_to_file_path(&params.text_document.uri)
            && path.is_file()
            && params.text_document.language_id == gen_lsp_types::LanguageKind::Rust
        {
            let mut open_files = self.open_files.load().as_ref().clone();
            open_files = open_files.insert(path.clone());
            self.open_files.store(Arc::new(open_files));
            if self.add_analyze_target(&path).await {
                self.do_analyze().await;
            }
        }
    }

    async fn did_close(&self, params: gen_lsp_types::DidCloseTextDocumentParams) {
        if let Some(path) = utils::uri_to_file_path(&params.text_document.uri) {
            let mut open_files = self.open_files.load().as_ref().clone();
            open_files = open_files.remove(&path);
            self.open_files.store(Arc::new(open_files));
        }
    }

    async fn did_change(&self, params: gen_lsp_types::DidChangeTextDocumentParams) {
        let path = utils::uri_to_file_path(&params.text_document.text_document_identifier.uri);
        {
            match path {
                Some(path) => self.analyzed.invalidate_file(&path),
                // No usable URI: still supersede whatever is in flight.
                None => {
                    self.analyzed.start_generation();
                }
            }
        }
        self.shutdown_subprocesses().await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.shutdown_subprocesses().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Function;

    fn function(fn_id: u32) -> SharedPointer<Function, ArcK> {
        SharedPointer::new(Function {
            fn_id,
            name: format!("f{fn_id}"),
            basic_blocks: Vec::new(),
            decls: Vec::new(),
        })
    }

    fn delivery(path: &str, item: SharedPointer<Function, ArcK>) -> Workspace {
        let key: Arc<str> = Arc::from(path);
        let file: SharedPointer<File, ArcK> = SharedPointer::new(File::new().push(item));
        let krate: SharedPointer<Crate, ArcK> =
            SharedPointer::new(Crate([(key, file)].into_iter().collect()));
        let name: Arc<str> = Arc::from("krate");
        Workspace([(name, krate)].into_iter().collect())
    }

    fn folded(snapshot: &Snapshot, target: &str, ws: &Workspace) -> Snapshot {
        snapshot
            .merged(target, ws)
            .expect("a delivery carries something")
    }

    #[test]
    fn a_file_is_found_by_its_key() {
        let snapshot = folded(
            &folded(
                &Snapshot::empty(),
                "/ws-a",
                &delivery("/ws-a/src/lib.rs", function(1)),
            ),
            "/ws-b",
            &delivery("/ws-b/src/main.rs", function(2)),
        );

        let found = snapshot.files_of(Path::new("/ws-b/src/main.rs"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].items[0].fn_id, 2);
    }

    #[test]
    fn a_file_shared_by_two_workspaces_keeps_both_entries() {
        let snapshot = folded(
            &folded(
                &Snapshot::empty(),
                "/ws",
                &delivery("/ws/sub/src/lib.rs", function(1)),
            ),
            "/ws/sub",
            &delivery("/ws/sub/src/lib.rs", function(2)),
        );

        let found = snapshot.files_of(Path::new("/ws/sub/src/lib.rs"));
        assert_eq!(
            found.len(),
            2,
            "one entry per workspace, not one global entry"
        );
    }

    #[test]
    fn re_delivery_under_the_same_workspace_replaces() {
        let fresh = function(1);
        let snapshot = folded(
            &folded(
                &Snapshot::empty(),
                "/ws",
                &delivery("/ws/src/lib.rs", function(1)),
            ),
            "/ws",
            &delivery("/ws/src/lib.rs", SharedPointer::clone(&fresh)),
        );

        let found = snapshot.files_of(Path::new("/ws/src/lib.rs"));
        assert_eq!(found.len(), 1);
        assert!(
            SharedPointer::ptr_eq(&found[0].items[0], &fresh),
            "the fresh delivery replaces the stale one"
        );
    }

    #[test]
    fn an_empty_snapshot_reads_as_not_analysed() {
        assert!(!Snapshot::empty().has_files());
        let snapshot = folded(
            &Snapshot::empty(),
            "/ws",
            &delivery("/ws/src/lib.rs", function(1)),
        );
        assert!(snapshot.has_files());
    }
}
