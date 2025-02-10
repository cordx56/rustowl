mod toolchain_version;
mod utils;

use mktemp::Temp;
use models::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process,
    sync::RwLock,
    task::JoinSet,
};
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use toolchain_version::TOOLCHAIN_VERSION;

#[allow(unused)]
#[derive(serde::Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Deco<R = Range> {
    Lifetime {
        local: Local,
        range: R,
        hover_text: String,
        is_display: bool,
    },
    ImmBorrow {
        local: Local,
        range: R,
        hover_text: String,
        is_display: bool,
    },
    MutBorrow {
        local: Local,
        range: R,
        hover_text: String,
        is_display: bool,
    },
    Move {
        local: Local,
        range: R,
        hover_text: String,
        is_display: bool,
    },
    Call {
        local: Local,
        range: R,
        hover_text: String,
        is_display: bool,
    },
    Outlive {
        local: Local,
        range: R,
        hover_text: String,
        is_display: bool,
    },
}
impl Deco<Range> {
    fn to_lsp_range(&self, s: &str) -> Deco<lsp_types::Range> {
        match self.clone() {
            Deco::Lifetime {
                local,
                range,
                hover_text,
                is_display,
            } => {
                let start = utils::index_to_line_char(s, range.from.0);
                let end = utils::index_to_line_char(s, range.until.0);
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Lifetime {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    is_display,
                }
            }
            Deco::ImmBorrow {
                local,
                range,
                hover_text,
                is_display,
            } => {
                let start = utils::index_to_line_char(s, range.from.0);
                let end = utils::index_to_line_char(s, range.until.0);
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::ImmBorrow {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    is_display,
                }
            }
            Deco::MutBorrow {
                local,
                range,
                hover_text,
                is_display,
            } => {
                let start = utils::index_to_line_char(s, range.from.0);
                let end = utils::index_to_line_char(s, range.until.0);
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::MutBorrow {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    is_display,
                }
            }
            Deco::Move {
                local,
                range,
                hover_text,
                is_display,
            } => {
                let start = utils::index_to_line_char(s, range.from.0);
                let end = utils::index_to_line_char(s, range.until.0);
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Move {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    is_display,
                }
            }
            Deco::Call {
                local,
                range,
                hover_text,
                is_display,
            } => {
                let start = utils::index_to_line_char(s, range.from.0);
                let end = utils::index_to_line_char(s, range.until.0);
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Call {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    is_display,
                }
            }
            Deco::Outlive {
                local,
                range,
                hover_text,
                is_display,
            } => {
                let start = utils::index_to_line_char(s, range.from.0);
                let end = utils::index_to_line_char(s, range.until.0);
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Outlive {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    is_display,
                }
            }
        }
    }
}
#[derive(serde::Serialize, Clone, Debug)]
struct Decorations {
    is_analyzed: bool,
    path: Option<PathBuf>,
    decorations: Vec<Deco<lsp_types::Range>>,
}
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
struct CursorRequest {
    position: lsp_types::Position,
    document: lsp_types::TextDocumentIdentifier,
}

#[derive(Clone, Debug)]
struct SelectLocal {
    pos: Loc,
    selected: Vec<Local>,
    current_fn_id: u32,
}
impl SelectLocal {
    fn new(pos: Loc) -> Self {
        Self {
            pos,
            selected: Vec::new(),
            current_fn_id: 0,
        }
    }
    fn select(&mut self, local_id: u32, range: Range) {
        if range.from <= self.pos && self.pos <= range.until {
            self.selected.push(Local::new(local_id, self.current_fn_id));
        }
    }
}
impl utils::MirVisitor for SelectLocal {
    fn visit_decl(&mut self, decl: &MirDecl) {
        if let MirDecl::User {
            local_index,
            fn_id,
            span,
            ..
        } = decl
        {
            self.current_fn_id = *fn_id;
            self.select(*local_index, *span);
        }
    }
    fn visit_stmt(&mut self, stmt: &MirStatement) {
        if let MirStatement::Assign { rval, .. } = stmt {
            match rval {
                Some(MirRval::Move {
                    target_local_index,
                    range,
                }) => {
                    self.select(*target_local_index, *range);
                }
                Some(MirRval::Borrow {
                    target_local_index,
                    range,
                    ..
                }) => {
                    self.select(*target_local_index, *range);
                }
                _ => {}
            }
        }
    }
    fn visit_term(&mut self, term: &MirTerminator) {
        if let MirTerminator::Call {
            destination_local_index,
            fn_span,
        } = term
        {
            self.select(*destination_local_index, *fn_span);
        }
    }
}
#[derive(Clone, Debug)]
struct CalcDecos {
    locals: Vec<Local>,
    decorations: Vec<Deco>,
    current_fn_id: u32,
}
impl CalcDecos {
    pub fn new(locals: Vec<Local>) -> Self {
        Self {
            locals,
            decorations: Vec::new(),
            current_fn_id: 0,
        }
    }

    fn get_deco_order(deco: &Deco) -> u8 {
        match deco {
            Deco::Lifetime { .. } => 0,
            Deco::ImmBorrow { .. } => 1,
            Deco::MutBorrow { .. } => 2,
            Deco::Move { .. } => 3,
            Deco::Call { .. } => 4,
            Deco::Outlive { .. } => 5,
        }
    }

    fn sort_by_definition(&mut self) {
        self.decorations.sort_by_key(Self::get_deco_order);
    }

    fn handle_overlapping(&mut self) {
        self.sort_by_definition();
        let mut i = 1;
        while i < self.decorations.len() {
            let current_range = match &self.decorations[i] {
                Deco::Lifetime { range, .. }
                | Deco::ImmBorrow { range, .. }
                | Deco::MutBorrow { range, .. }
                | Deco::Move { range, .. }
                | Deco::Call { range, .. }
                | Deco::Outlive { range, .. } => *range,
            };

            let mut j = 0;
            while j < i {
                let prev = &self.decorations[j];
                let (prev_range, prev_is_display) = match prev {
                    Deco::Lifetime {
                        range, is_display, ..
                    }
                    | Deco::ImmBorrow {
                        range, is_display, ..
                    }
                    | Deco::MutBorrow {
                        range, is_display, ..
                    }
                    | Deco::Move {
                        range, is_display, ..
                    }
                    | Deco::Call {
                        range, is_display, ..
                    }
                    | Deco::Outlive {
                        range, is_display, ..
                    } => (*range, *is_display),
                };

                if !prev_is_display {
                    j += 1;
                    continue;
                }

                if let Some(common) = utils::common_range(current_range, prev_range) {
                    if common.from < common.until {
                        let mut new_decos = Vec::new();
                        let non_overlapping = utils::exclude_ranges(vec![prev_range], vec![common]);

                        for range in non_overlapping {
                            let new_deco = match prev {
                                Deco::Lifetime {
                                    local, hover_text, ..
                                } => Deco::Lifetime {
                                    local: *local,
                                    range,
                                    hover_text: hover_text.clone(),
                                    is_display: true,
                                },
                                Deco::ImmBorrow {
                                    local, hover_text, ..
                                } => Deco::ImmBorrow {
                                    local: *local,
                                    range,
                                    hover_text: hover_text.clone(),
                                    is_display: true,
                                },
                                Deco::MutBorrow {
                                    local, hover_text, ..
                                } => Deco::MutBorrow {
                                    local: *local,
                                    range,
                                    hover_text: hover_text.clone(),
                                    is_display: true,
                                },
                                Deco::Move {
                                    local, hover_text, ..
                                } => Deco::Move {
                                    local: *local,
                                    range,
                                    hover_text: hover_text.clone(),
                                    is_display: true,
                                },
                                Deco::Call {
                                    local, hover_text, ..
                                } => Deco::Call {
                                    local: *local,
                                    range,
                                    hover_text: hover_text.clone(),
                                    is_display: true,
                                },
                                Deco::Outlive {
                                    local, hover_text, ..
                                } => Deco::Outlive {
                                    local: *local,
                                    range,
                                    hover_text: hover_text.clone(),
                                    is_display: true,
                                },
                            };
                            new_decos.push(new_deco);
                        }

                        match &mut self.decorations[j] {
                            Deco::Lifetime {
                                range, is_display, ..
                            }
                            | Deco::ImmBorrow {
                                range, is_display, ..
                            }
                            | Deco::MutBorrow {
                                range, is_display, ..
                            }
                            | Deco::Move {
                                range, is_display, ..
                            }
                            | Deco::Call {
                                range, is_display, ..
                            }
                            | Deco::Outlive {
                                range, is_display, ..
                            } => {
                                *range = common;
                                *is_display = false;
                            }
                        }

                        for (jj, deco) in new_decos.into_iter().enumerate() {
                            self.decorations.insert(j + jj + 1, deco);
                        }
                        self.sort_by_definition();
                    }
                }
                j += 1;
            }
            i += 1;
        }
    }
}
impl utils::MirVisitor for CalcDecos {
    fn visit_decl(&mut self, decl: &MirDecl) {
        if let MirDecl::User {
            local_index,
            fn_id,
            lives,
            drop_range,
            must_live_at,
            name,
            ..
        } = decl
        {
            self.current_fn_id = *fn_id;
            let local = Local::new(*local_index, *fn_id);
            if self.locals.contains(&local) {
                // merge Drop object lives
                let mut drop_copy_live = lives.clone();
                drop_copy_live.extend_from_slice(drop_range);
                drop_copy_live = utils::eliminated_ranges(drop_copy_live.clone());
                for range in &drop_copy_live {
                    self.decorations.push(Deco::Lifetime {
                        local,
                        range: *range,
                        hover_text: format!("lifetime of variable `{}`", name),
                        is_display: true,
                    });
                }
                let outlive = utils::exclude_ranges(must_live_at.clone(), drop_copy_live);
                for range in outlive {
                    self.decorations.push(Deco::Outlive {
                        local,
                        range,
                        hover_text: format!("variable `{}` is required to live here", name),
                        is_display: true,
                    });
                }
            }
        }
    }
    fn visit_stmt(&mut self, stmt: &MirStatement) {
        if let MirStatement::Assign { rval, .. } = stmt {
            match rval {
                Some(MirRval::Move {
                    target_local_index,
                    range,
                }) => {
                    let local = Local::new(*target_local_index, self.current_fn_id);
                    if self.locals.contains(&local) {
                        self.decorations.push(Deco::Move {
                            local,
                            range: *range,
                            hover_text: "variable moved".to_string(),
                            is_display: true,
                        });
                    }
                }
                Some(MirRval::Borrow {
                    target_local_index,
                    range,
                    mutable,
                    ..
                }) => {
                    let local = Local::new(*target_local_index, self.current_fn_id);
                    if self.locals.contains(&local) {
                        if *mutable {
                            self.decorations.push(Deco::MutBorrow {
                                local,
                                range: *range,
                                hover_text: "mutable borrow".to_string(),
                                is_display: true,
                            });
                        } else {
                            self.decorations.push(Deco::ImmBorrow {
                                local,
                                range: *range,
                                hover_text: "immutable borrow".to_string(),
                                is_display: true,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    fn visit_term(&mut self, term: &MirTerminator) {
        if let MirTerminator::Call {
            destination_local_index,
            fn_span,
        } = term
        {
            let local = Local::new(*destination_local_index, self.current_fn_id);
            if self.locals.contains(&local) {
                let mut i = 0;
                for deco in &self.decorations {
                    if let Deco::Call { range, .. } = deco {
                        if utils::is_super_range(*fn_span, *range) {
                            return;
                        }
                    }
                }
                while i < self.decorations.len() {
                    let range = match &self.decorations[i] {
                        Deco::Call { range, .. } => Some(range),
                        _ => None,
                    };
                    if let Some(range) = range {
                        if utils::is_super_range(*range, *fn_span) {
                            self.decorations.remove(i);
                            continue;
                        }
                    }
                    i += 1;
                }
                self.decorations.push(Deco::Call {
                    local,
                    range: *fn_span,
                    hover_text: "function call".to_string(),
                    is_display: true,
                });
            }
        }
    }
}

fn search_cargo(p: &PathBuf) -> Vec<PathBuf> {
    let mut res = Vec::new();
    if let Ok(mut paths) = std::fs::read_dir(p) {
        while let Some(Ok(path)) = paths.next() {
            if let Ok(meta) = path.metadata() {
                if meta.is_dir() {
                    res.extend_from_slice(&search_cargo(&path.path()));
                } else if path.file_name() == "Cargo.toml" {
                    let dir = path.path().parent().unwrap().to_path_buf();
                    res.push(dir);
                }
            }
        }
    }
    res
}

#[derive(Debug)]
struct Backend {
    #[allow(unused)]
    client: Client,
    roots: Arc<RwLock<HashMap<PathBuf, PathBuf>>>,
    analyzed: Arc<RwLock<Option<Workspace>>>,
    processes: Arc<RwLock<JoinSet<()>>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            roots: Arc::new(RwLock::new(HashMap::new())),
            analyzed: Arc::new(RwLock::new(None)),
            processes: Arc::new(RwLock::new(JoinSet::new())),
        }
    }
    async fn set_roots(&self, uri: &lsp_types::Url) {
        let path = uri.to_file_path().unwrap();
        let mut write = self.roots.write().await;
        'entries: for path in search_cargo(&path) {
            for (root, target) in write.clone().into_iter() {
                if root.starts_with(&path) {
                    write.remove(&root);
                    write.insert(path, target);
                    continue 'entries;
                } else if path.starts_with(&root) {
                    continue 'entries;
                }
            }
            write.insert(path, Temp::new_dir().unwrap().to_path_buf());
        }
    }

    async fn analzye(&self) {
        // wait for rust-analyzer
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        {
            *self.analyzed.write().await = None;
        }
        let roots = { self.roots.read().await.clone() };
        let mut join = self.processes.write().await;
        join.shutdown().await;
        for (root, target) in roots {
            let mut child = process::Command::new("rustup")
                .arg("run")
                .arg(TOOLCHAIN_VERSION)
                .arg("cargo")
                .arg("owl")
                .arg(&root)
                .arg(&target)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
            let analyzed = self.analyzed.clone();
            tokio::spawn(async move {
                while let Ok(Some(line)) = stdout.next_line().await {
                    if let Ok(ws) = serde_json::from_str::<Workspace>(&line) {
                        let write = &mut *analyzed.write().await;
                        if let Some(write) = write {
                            *write = write.clone().merge(ws);
                        } else {
                            *write = Some(ws);
                        }
                    }
                }
            });
            join.spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
    async fn cleanup_targets(&self) {
        for (_, target) in self.roots.read().await.iter() {
            std::fs::remove_dir_all(target).ok();
        }
    }

    async fn decos(&self, filepath: &Path, position: Loc) -> Vec<Deco> {
        let mut selected = SelectLocal::new(position);
        if let Some(analyzed) = &*self.analyzed.read().await {
            for (mir_filename, file) in analyzed.0.iter() {
                if filepath.ends_with(mir_filename) {
                    for item in &file.items {
                        utils::mir_visit(item, &mut selected);
                    }
                }
            }

            let mut calc = CalcDecos::new(selected.selected);
            for (mir_filename, file) in analyzed.0.iter() {
                if filepath.ends_with(mir_filename) {
                    for item in &file.items {
                        utils::mir_visit(item, &mut calc);
                    }
                }
            }
            calc.handle_overlapping();
            calc.decorations
        } else {
            Vec::new()
        }
    }

    async fn cursor(&self, params: CursorRequest) -> jsonrpc::Result<Decorations> {
        let is_analyzed = self.analyzed.read().await.is_some();
        if let Ok(path) = params.document.uri.to_file_path() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let pos = Loc(utils::line_char_to_index(
                    &text,
                    params.position.line,
                    params.position.character,
                ));
                let decos = self.decos(&path, pos).await;
                let decorations = decos.into_iter().map(|v| v.to_lsp_range(&text)).collect();
                return Ok(Decorations {
                    is_analyzed,
                    path: Some(path),
                    decorations,
                });
            }
        }
        Ok(Decorations {
            is_analyzed,
            path: None,
            decorations: Vec::new(),
        })
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
                self.set_roots(&ws.uri).await;
            }
        }
        let sync_options = lsp_types::TextDocumentSyncOptions {
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
        Ok(init_res)
    }
    async fn initialized(&self, _p: lsp_types::InitializedParams) {
        self.analzye().await;
    }
    async fn did_save(&self, _params: lsp_types::DidSaveTextDocumentParams) {
        self.analzye().await;
    }
    async fn did_change(&self, _params: lsp_types::DidChangeTextDocumentParams) {
        *self.analyzed.write().await = None;
        self.processes.write().await.shutdown().await;
    }

    async fn did_change_workspace_folders(
        &self,
        params: lsp_types::DidChangeWorkspaceFoldersParams,
    ) -> () {
        for added in params.event.added {
            self.set_roots(&added.uri).await;
        }
        self.analzye().await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.cleanup_targets().await;
        self.processes.write().await.shutdown().await;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("rustowl/cursor", Backend::cursor)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}
