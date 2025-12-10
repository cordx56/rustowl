use crate::models::FoldIndexSet as HashSet;
use crate::{lsp::progress, models::*, utils};
use std::path::PathBuf;
use tower_lsp_server::ls_types as lsp_types;

// Variable names that should be filtered out during analysis
const ASYNC_MIR_VARS: [&str; 2] = ["_task_context", "__awaitee"];
const ASYNC_RESUME_TY: [&str; 2] = [
    "std::future::ResumeTy",
    "impl std::future::Future<Output = ()>",
];

#[derive(serde::Serialize, PartialEq, Eq, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Deco<R = Range> {
    Lifetime {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    ImmBorrow {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    MutBorrow {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    Move {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    Call {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    SharedMut {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    Outlive {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
}
impl Deco<Range> {
    pub fn to_lsp_range(&self, s: &str) -> Deco<lsp_types::Range> {
        match self.clone() {
            Deco::Lifetime {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
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
                    overlapped,
                }
            }
            Deco::ImmBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
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
                    overlapped,
                }
            }
            Deco::MutBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
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
                    overlapped,
                }
            }
            Deco::Move {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
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
                    overlapped,
                }
            }
            Deco::Call {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
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
                    overlapped,
                }
            }
            Deco::SharedMut {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::SharedMut {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }

            Deco::Outlive {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = utils::index_to_line_char(s, range.from());
                let end = utils::index_to_line_char(s, range.until());
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
                    overlapped,
                }
            }
        }
    }
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct Decorations {
    pub is_analyzed: bool,
    pub status: progress::AnalysisStatus,
    pub path: Option<PathBuf>,
    pub decorations: Vec<Deco<lsp_types::Range>>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CursorRequest {
    pub position: lsp_types::Position,
    pub document: lsp_types::TextDocumentIdentifier,
}
impl CursorRequest {
    pub fn path(&self) -> Option<PathBuf> {
        self.document.uri.to_file_path().map(|p| p.into_owned())
    }
    pub fn position(&self) -> lsp_types::Position {
        self.position
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SelectReason {
    Var,
    Move,
    Borrow,
    Call,
}
#[derive(Clone, Debug)]
pub struct SelectLocal {
    pos: Loc,
    candidate_local_decls: Vec<FnLocal>,
    selected: Option<(SelectReason, FnLocal, Range)>,
}
impl SelectLocal {
    pub fn new(pos: Loc) -> Self {
        Self {
            pos,
            candidate_local_decls: Vec::new(),
            selected: None,
        }
    }

    fn select(&mut self, reason: SelectReason, local: FnLocal, range: Range) {
        if !self.candidate_local_decls.contains(&local) {
            return;
        }
        if range.from() <= self.pos && self.pos <= range.until() {
            if let Some((old_reason, _, old_range)) = self.selected {
                match (old_reason, reason) {
                    (_, SelectReason::Var) => {
                        if range.size() < old_range.size() {
                            self.selected = Some((reason, local, range));
                        }
                    }
                    (SelectReason::Var, _) => {}
                    (_, SelectReason::Move) | (_, SelectReason::Borrow) => {
                        if range.size() < old_range.size() {
                            self.selected = Some((reason, local, range));
                        }
                    }
                    (SelectReason::Call, SelectReason::Call) => {
                        // Select narrower range for method calls (prefer tighter spans)
                        if range.size() < old_range.size() {
                            self.selected = Some((reason, local, range));
                        }
                    }
                    _ => {}
                }
            } else {
                self.selected = Some((reason, local, range));
            }
        }
    }

    pub fn selected(&self) -> Option<FnLocal> {
        self.selected.map(|v| v.1)
    }
}
impl utils::MirVisitor for SelectLocal {
    fn visit_decl(&mut self, decl: &MirDecl) {
        let (local, ty, name) = match decl {
            MirDecl::User {
                local, ty, name, ..
            } => (local, ty, Some(name)),
            MirDecl::Other { local, ty, .. } => (local, ty, None),
        };

        // Filter out async-related types
        if ASYNC_RESUME_TY.contains(&ty.as_str()) {
            return;
        }

        // Filter out async-related variable names
        if let Some(var_name) = name
            && ASYNC_MIR_VARS.contains(&var_name.as_str())
        {
            return;
        }

        self.candidate_local_decls.push(*local);
        if let MirDecl::User { local, span, .. } = decl {
            self.select(SelectReason::Var, *local, *span);
        }
    }
    fn visit_stmt(&mut self, stmt: &MirStatement) {
        if let MirStatement::Assign { rval, .. } = stmt {
            match rval {
                Some(MirRval::Move {
                    target_local,
                    range,
                }) => {
                    self.select(SelectReason::Move, *target_local, *range);
                }
                Some(MirRval::Borrow {
                    target_local,
                    range,
                    ..
                }) => {
                    self.select(SelectReason::Borrow, *target_local, *range);
                }
                _ => {}
            }
        }
    }
    fn visit_term(&mut self, term: &MirTerminator) {
        if let MirTerminator::Call {
            destination_local,
            fn_span,
        } = term
        {
            self.select(SelectReason::Call, *destination_local, *fn_span);
        }
    }
}
#[derive(Clone, Debug)]
pub struct CalcDecos {
    locals: HashSet<FnLocal>,
    decorations: Vec<Deco>,
    current_fn_id: u32,
}
impl CalcDecos {
    pub fn new(locals: impl IntoIterator<Item = FnLocal>) -> Self {
        Self {
            locals: locals.into_iter().collect(),
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
            Deco::SharedMut { .. } => 5,
            Deco::Outlive { .. } => 6,
        }
    }

    fn sort_by_definition(&mut self) {
        self.decorations.sort_by_key(Self::get_deco_order);
    }

    pub fn handle_overlapping(&mut self) {
        self.sort_by_definition();
        let mut i = 1;
        'outer: while i < self.decorations.len() {
            let current_range = match &self.decorations[i] {
                Deco::Lifetime { range, .. }
                | Deco::ImmBorrow { range, .. }
                | Deco::MutBorrow { range, .. }
                | Deco::Move { range, .. }
                | Deco::Call { range, .. }
                | Deco::SharedMut { range, .. }
                | Deco::Outlive { range, .. } => *range,
            };

            let mut j = 0;
            while j < i {
                let prev = &self.decorations[j];
                if prev == &self.decorations[i] {
                    self.decorations.remove(i);
                    continue 'outer;
                }
                let (prev_range, prev_overlapped) = match prev {
                    Deco::Lifetime {
                        range, overlapped, ..
                    }
                    | Deco::ImmBorrow {
                        range, overlapped, ..
                    }
                    | Deco::MutBorrow {
                        range, overlapped, ..
                    }
                    | Deco::Move {
                        range, overlapped, ..
                    }
                    | Deco::Call {
                        range, overlapped, ..
                    }
                    | Deco::SharedMut {
                        range, overlapped, ..
                    }
                    | Deco::Outlive {
                        range, overlapped, ..
                    } => (*range, *overlapped),
                };

                if prev_overlapped {
                    j += 1;
                    continue;
                }

                if let Some(common) = utils::common_range(current_range, prev_range) {
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
                                overlapped: false,
                            },
                            Deco::ImmBorrow {
                                local, hover_text, ..
                            } => Deco::ImmBorrow {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::MutBorrow {
                                local, hover_text, ..
                            } => Deco::MutBorrow {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::Move {
                                local, hover_text, ..
                            } => Deco::Move {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::Call {
                                local, hover_text, ..
                            } => Deco::Call {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::SharedMut {
                                local, hover_text, ..
                            } => Deco::SharedMut {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::Outlive {
                                local, hover_text, ..
                            } => Deco::Outlive {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                        };
                        new_decos.push(new_deco);
                    }

                    match &mut self.decorations[j] {
                        Deco::Lifetime {
                            range, overlapped, ..
                        }
                        | Deco::ImmBorrow {
                            range, overlapped, ..
                        }
                        | Deco::MutBorrow {
                            range, overlapped, ..
                        }
                        | Deco::Move {
                            range, overlapped, ..
                        }
                        | Deco::Call {
                            range, overlapped, ..
                        }
                        | Deco::SharedMut {
                            range, overlapped, ..
                        }
                        | Deco::Outlive {
                            range, overlapped, ..
                        } => {
                            *range = common;
                            *overlapped = true;
                        }
                    }

                    for (jj, deco) in new_decos.into_iter().enumerate() {
                        self.decorations.insert(j + jj + 1, deco);
                    }
                }
                j += 1;
            }
            i += 1;
        }
    }

    pub fn decorations(self) -> Vec<Deco> {
        self.decorations
    }
}
impl utils::MirVisitor for CalcDecos {
    fn visit_decl(&mut self, decl: &MirDecl) {
        let (local, lives, shared_borrow, mutable_borrow, drop_range, must_live_at, name, drop) =
            match decl {
                MirDecl::User {
                    local,
                    name,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    drop,
                    ..
                } => (
                    *local,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    Some(name),
                    drop,
                ),
                MirDecl::Other {
                    local,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    drop,
                    ..
                } => (
                    *local,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    None,
                    drop,
                ),
            };
        self.current_fn_id = local.fn_id;
        if self.locals.contains(&local) {
            let var_str = match name {
                Some(mir_var_name) => {
                    format!("variable `{mir_var_name}`")
                }
                None => "anonymous variable".to_owned(),
            };
            // merge Drop object lives
            let drop_copy_live = if *drop {
                utils::eliminated_ranges_small(drop_range.clone())
            } else {
                utils::eliminated_ranges_small(lives.clone())
            };
            for range in &drop_copy_live {
                self.decorations.push(Deco::Lifetime {
                    local,
                    range: *range,
                    hover_text: format!("lifetime of {var_str}"),
                    overlapped: false,
                });
            }
            let mut borrow_ranges = Vec::with_capacity(shared_borrow.len() + mutable_borrow.len());
            borrow_ranges.extend(shared_borrow.iter().copied());
            borrow_ranges.extend(mutable_borrow.iter().copied());
            let shared_mut = utils::common_ranges(&borrow_ranges);
            for range in shared_mut {
                self.decorations.push(Deco::SharedMut {
                    local,
                    range,
                    hover_text: format!("immutable and mutable borrows of {var_str} exist here"),
                    overlapped: false,
                });
            }
            let outlive = utils::exclude_ranges_small(must_live_at.clone(), drop_copy_live);
            for range in outlive {
                self.decorations.push(Deco::Outlive {
                    local,
                    range,
                    hover_text: format!("{var_str} is required to live here"),
                    overlapped: false,
                });
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &MirStatement) {
        if let MirStatement::Assign { rval, .. } = stmt {
            match rval {
                Some(MirRval::Move {
                    target_local,
                    range,
                }) => {
                    if self.locals.contains(target_local) {
                        self.decorations.push(Deco::Move {
                            local: *target_local,
                            range: *range,
                            hover_text: "variable moved".to_string(),
                            overlapped: false,
                        });
                    }
                }
                Some(MirRval::Borrow {
                    target_local,
                    range,
                    mutable,
                    ..
                }) => {
                    if self.locals.contains(target_local) {
                        if *mutable {
                            self.decorations.push(Deco::MutBorrow {
                                local: *target_local,
                                range: *range,
                                hover_text: "mutable borrow".to_string(),
                                overlapped: false,
                            });
                        } else {
                            self.decorations.push(Deco::ImmBorrow {
                                local: *target_local,
                                range: *range,
                                hover_text: "immutable borrow".to_string(),
                                overlapped: false,
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
            destination_local,
            fn_span,
        } = term
            && self.locals.contains(destination_local)
        {
            let mut i = 0;
            for deco in &self.decorations {
                if let Deco::Call { range, .. } = deco
                    && utils::is_super_range(*fn_span, *range)
                {
                    return;
                }
            }
            while i < self.decorations.len() {
                let range = match &self.decorations[i] {
                    Deco::Call { range, .. } => Some(range),
                    _ => None,
                };
                if let Some(range) = range
                    && utils::is_super_range(*range, *fn_span)
                {
                    self.decorations.remove(i);
                    continue;
                }
                i += 1;
            }
            self.decorations.push(Deco::Call {
                local: *destination_local,
                range: *fn_span,
                hover_text: "function call".to_string(),
                overlapped: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FnLocal, Loc, MirDecl, Range};
    use crate::utils::MirVisitor;
    use smallvec::SmallVec;

    #[test]
    fn test_async_variable_filtering() {
        let mut selector = SelectLocal::new(Loc(10));

        // Test that async variables are filtered out
        let mut lives_vec: SmallVec<[Range; 4]> = SmallVec::new();
        lives_vec.push(Range::new(Loc(0), Loc(20)).unwrap());

        let mut drop_range_vec: SmallVec<[Range; 4]> = SmallVec::new();
        drop_range_vec.push(Range::new(Loc(15), Loc(25)).unwrap());

        let async_var_decl = MirDecl::User {
            local: FnLocal::new(1, 1),
            name: "_task_context".into(),
            ty: "i32".into(),
            lives: lives_vec,
            shared_borrow: SmallVec::new(),
            mutable_borrow: SmallVec::new(),
            drop_range: drop_range_vec,
            must_live_at: SmallVec::new(),
            drop: false,
            span: Range::new(Loc(5), Loc(15)).unwrap(),
        };

        selector.visit_decl(&async_var_decl);

        // The async variable should be filtered out, so no candidates should be added
        assert!(selector.candidate_local_decls.is_empty());
    }

    #[test]
    fn test_regular_variable_not_filtered() {
        let mut selector = SelectLocal::new(Loc(10));

        // Test that regular variables are not filtered out
        let mut lives_vec: SmallVec<[Range; 4]> = SmallVec::new();
        lives_vec.push(Range::new(Loc(0), Loc(20)).unwrap());

        let mut drop_range_vec: SmallVec<[Range; 4]> = SmallVec::new();
        drop_range_vec.push(Range::new(Loc(15), Loc(25)).unwrap());

        let regular_var_decl = MirDecl::User {
            local: FnLocal::new(1, 1),
            name: "my_var".into(),
            ty: "i32".into(),
            lives: lives_vec,
            shared_borrow: SmallVec::new(),
            mutable_borrow: SmallVec::new(),
            drop_range: drop_range_vec,
            must_live_at: SmallVec::new(),
            drop: false,
            span: Range::new(Loc(5), Loc(15)).unwrap(),
        };

        selector.visit_decl(&regular_var_decl);

        // The regular variable should not be filtered out
        assert_eq!(selector.candidate_local_decls.len(), 1);
        assert_eq!(selector.candidate_local_decls[0], FnLocal::new(1, 1));
    }

    #[test]
    fn test_call_selection_prefers_narrower_range() {
        let mut selector = SelectLocal::new(Loc(10));
        let local = FnLocal::new(1, 1);

        // Add local to candidates
        selector.candidate_local_decls.push(local);

        // First call with wider range
        let wide_range = Range::new(Loc(5), Loc(20)).unwrap();
        selector.select(SelectReason::Call, local, wide_range);

        // Second call with narrower range
        let narrow_range = Range::new(Loc(8), Loc(15)).unwrap();
        selector.select(SelectReason::Call, local, narrow_range);

        // Should select the narrower range (method call preference)
        let selected = selector.selected();
        assert_eq!(selected, Some(local));

        // Verify the selected range is the narrower one
        if let Some((reason, _, range)) = selector.selected {
            assert_eq!(reason, SelectReason::Call);
            assert_eq!(range, narrow_range);
        }
    }

    #[test]
    fn test_decoration_creation() {
        let locals = vec![FnLocal::new(1, 1)];
        let mut calc = CalcDecos::new(locals);

        let mut lives_vec: SmallVec<[Range; 4]> = SmallVec::new();
        lives_vec.push(Range::new(Loc(0), Loc(20)).unwrap());

        let mut drop_range_vec: SmallVec<[Range; 4]> = SmallVec::new();
        drop_range_vec.push(Range::new(Loc(15), Loc(25)).unwrap());

        let decl = MirDecl::User {
            local: FnLocal::new(1, 1),
            name: "test_var".into(),
            ty: "i32".into(),
            lives: lives_vec,
            shared_borrow: SmallVec::new(),
            mutable_borrow: SmallVec::new(),
            drop_range: drop_range_vec,
            must_live_at: SmallVec::new(),
            drop: false,
            span: Range::new(Loc(5), Loc(15)).unwrap(),
        };

        calc.visit_decl(&decl);

        let decorations = calc.decorations();
        // Should have at least one decoration (lifetime)
        assert!(!decorations.is_empty());
    }

    #[test]
    fn test_select_local_new() {
        let pos = Loc(10);
        let selector = SelectLocal::new(pos);

        assert_eq!(selector.pos, pos);
        assert!(selector.candidate_local_decls.is_empty());
        assert!(selector.selected.is_none());
    }

    #[test]
    fn test_select_local_select_var() {
        let mut selector = SelectLocal::new(Loc(10));
        let local = FnLocal::new(1, 1);
        let range = Range::new(Loc(5), Loc(15)).unwrap();

        // Add local to candidates
        selector.candidate_local_decls.push(local);

        // Select with Var reason
        selector.select(SelectReason::Var, local, range);

        assert!(selector.selected.is_some());
        if let Some((reason, selected_local, selected_range)) = selector.selected {
            assert_eq!(reason, SelectReason::Var);
            assert_eq!(selected_local, local);
            assert_eq!(selected_range, range);
        }
    }

    #[test]
    fn test_calc_decos_new() {
        let locals = vec![FnLocal::new(1, 1), FnLocal::new(2, 1)];
        let calc = CalcDecos::new(locals.clone());

        assert_eq!(calc.locals.len(), 2);
        assert!(calc.decorations.is_empty());
        assert_eq!(calc.current_fn_id, 0);
    }

    #[test]
    fn test_calc_decos_get_deco_order() {
        // Test decoration ordering
        let lifetime_deco = Deco::Lifetime {
            local: FnLocal::new(1, 1),
            range: Range::new(Loc(0), Loc(10)).unwrap(),
            hover_text: "test".to_string(),
            overlapped: false,
        };

        let borrow_deco = Deco::ImmBorrow {
            local: FnLocal::new(1, 1),
            range: Range::new(Loc(0), Loc(10)).unwrap(),
            hover_text: "test".to_string(),
            overlapped: false,
        };

        assert_eq!(CalcDecos::get_deco_order(&lifetime_deco), 0);
        assert_eq!(CalcDecos::get_deco_order(&borrow_deco), 1);
    }

    #[test]
    fn test_calc_decos_sort_by_definition() {
        let mut calc = CalcDecos::new(vec![]);

        // Add decorations in reverse order
        let call_deco = Deco::Call {
            local: FnLocal::new(1, 1),
            range: Range::new(Loc(0), Loc(10)).unwrap(),
            hover_text: "test".to_string(),
            overlapped: false,
        };

        let lifetime_deco = Deco::Lifetime {
            local: FnLocal::new(1, 1),
            range: Range::new(Loc(0), Loc(10)).unwrap(),
            hover_text: "test".to_string(),
            overlapped: false,
        };

        calc.decorations.push(call_deco);
        calc.decorations.push(lifetime_deco);

        calc.sort_by_definition();

        // After sorting, lifetime should come first (order 0)
        assert!(matches!(calc.decorations[0], Deco::Lifetime { .. }));
        assert!(matches!(calc.decorations[1], Deco::Call { .. }));
    }

    #[test]
    fn test_cursor_request_path() {
        let document = lsp_types::TextDocumentIdentifier {
            uri: "file:///test.rs".parse().unwrap(),
        };
        let request = CursorRequest {
            position: lsp_types::Position {
                line: 1,
                character: 5,
            },
            document,
        };

        let path = request.path();
        assert!(path.is_some());
        assert_eq!(path.unwrap().to_string_lossy(), "/test.rs");
    }

    #[test]
    fn test_cursor_request_position() {
        let position = lsp_types::Position {
            line: 10,
            character: 20,
        };
        let document = lsp_types::TextDocumentIdentifier {
            uri: "file:///test.rs".parse().unwrap(),
        };
        let request = CursorRequest { position, document };

        assert_eq!(request.position(), position);
    }
}
