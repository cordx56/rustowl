use crate::lsp::progress;
use crate::models::FoldIndexSet as HashSet;
use crate::models::{FnLocal, Loc, MirDecl, MirRval, MirStatement, MirTerminator, Range};
use crate::utils;
use std::path::PathBuf;
use tower_lsp_server::ls_types;

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
    pub fn to_lsp_range(&self, index: &utils::LineCharIndex) -> Deco<ls_types::Range> {
        match self {
            Deco::Lifetime {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Lifetime {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
                }
            }
            Deco::ImmBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::ImmBorrow {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
                }
            }
            Deco::MutBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::MutBorrow {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
                }
            }
            Deco::Move {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Move {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
                }
            }
            Deco::Call {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Call {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
                }
            }
            Deco::SharedMut {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::SharedMut {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
                }
            }

            Deco::Outlive {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = index.index_to_line_char(range.from());
                let end = index.index_to_line_char(range.until());
                let start = ls_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = ls_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Outlive {
                    local: *local,
                    range: ls_types::Range { start, end },
                    hover_text: hover_text.clone(),
                    overlapped: *overlapped,
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
    pub decorations: Vec<Deco<ls_types::Range>>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CursorRequest {
    pub position: ls_types::Position,
    pub document: ls_types::TextDocumentIdentifier,
}
impl CursorRequest {
    pub fn path(&self) -> Option<PathBuf> {
        self.document.uri.to_file_path().map(|p| p.into_owned())
    }
    pub fn position(&self) -> ls_types::Position {
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
                if self.decorations[j] == self.decorations[i] {
                    self.decorations.remove(i);
                    continue 'outer;
                }

                let (prev_range, prev_overlapped) = match &self.decorations[j] {
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
                    // Mark both decorations as overlapped on true intersection.
                    match &mut self.decorations[i] {
                        Deco::Lifetime { overlapped, .. }
                        | Deco::ImmBorrow { overlapped, .. }
                        | Deco::MutBorrow { overlapped, .. }
                        | Deco::Move { overlapped, .. }
                        | Deco::Call { overlapped, .. }
                        | Deco::SharedMut { overlapped, .. }
                        | Deco::Outlive { overlapped, .. } => {
                            *overlapped = true;
                        }
                    }

                    let mut new_decos = Vec::new();
                    let non_overlapping = utils::exclude_ranges(vec![prev_range], vec![common]);

                    for range in non_overlapping {
                        let new_deco = match &self.decorations[j] {
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
    use ecow::EcoVec;

    #[test]
    fn test_async_variable_filtering() {
        let mut selector = SelectLocal::new(Loc(10));

        // Test that async variables are filtered out
        let mut lives_vec: EcoVec<Range> = EcoVec::new();
        lives_vec.push(Range::new(Loc(0), Loc(20)).unwrap());

        let mut drop_range_vec: EcoVec<Range> = EcoVec::new();
        drop_range_vec.push(Range::new(Loc(15), Loc(25)).unwrap());

        let async_var_decl = MirDecl::User {
            local: FnLocal::new(1, 1),
            name: "_task_context".into(),
            ty: "i32".into(),
            lives: lives_vec,
            shared_borrow: EcoVec::new(),
            mutable_borrow: EcoVec::new(),
            drop_range: drop_range_vec,
            must_live_at: EcoVec::new(),
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
        let mut lives_vec: EcoVec<Range> = EcoVec::new();
        lives_vec.push(Range::new(Loc(0), Loc(20)).unwrap());

        let mut drop_range_vec: EcoVec<Range> = EcoVec::new();
        drop_range_vec.push(Range::new(Loc(15), Loc(25)).unwrap());

        let regular_var_decl = MirDecl::User {
            local: FnLocal::new(1, 1),
            name: "my_var".into(),
            ty: "i32".into(),
            lives: lives_vec,
            shared_borrow: EcoVec::new(),
            mutable_borrow: EcoVec::new(),
            drop_range: drop_range_vec,
            must_live_at: EcoVec::new(),
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
    fn select_local_ignores_non_candidates() {
        let mut selector = SelectLocal::new(Loc(10));
        let local = FnLocal::new(1, 1);

        // Not adding it to candidates means select() should ignore it.
        selector.select(
            SelectReason::Var,
            local,
            Range::new(Loc(0), Loc(20)).unwrap(),
        );

        assert!(selector.selected().is_none());
    }

    #[test]
    fn select_local_var_prefers_narrower_range() {
        let mut selector = SelectLocal::new(Loc(10));
        let local = FnLocal::new(1, 1);
        selector.candidate_local_decls.push(local);

        let wide = Range::new(Loc(0), Loc(20)).unwrap();
        let narrow = Range::new(Loc(8), Loc(11)).unwrap();

        selector.select(SelectReason::Var, local, wide);
        selector.select(SelectReason::Var, local, narrow);

        assert_eq!(selector.selected(), Some(local));
        let (reason, selected_local, selected_range) = selector.selected.unwrap();
        assert_eq!(reason, SelectReason::Var);
        assert_eq!(selected_local, local);
        assert_eq!(selected_range, narrow);
    }

    #[test]
    fn select_local_var_wins_over_borrow_selection() {
        let mut selector = SelectLocal::new(Loc(10));
        let local = FnLocal::new(1, 1);
        selector.candidate_local_decls.push(local);

        let borrow_range = Range::new(Loc(9), Loc(12)).unwrap();
        selector.select(SelectReason::Borrow, local, borrow_range);

        let var_range = Range::new(Loc(9), Loc(11)).unwrap();
        selector.select(SelectReason::Var, local, var_range);

        assert_eq!(selector.selected(), Some(local));
        let (reason, _, range) = selector.selected.unwrap();
        assert_eq!(reason, SelectReason::Var);
        assert_eq!(range, var_range);
    }

    #[test]
    fn calc_decos_dedupes_call_ranges() {
        let local = FnLocal::new(1, 1);

        // Candidate is populated by visiting its declaration.
        let decl = MirDecl::User {
            local,
            name: "x".into(),
            ty: "i32".into(),
            lives: EcoVec::new(),
            shared_borrow: EcoVec::new(),
            mutable_borrow: EcoVec::new(),
            drop_range: EcoVec::new(),
            must_live_at: EcoVec::new(),
            drop: false,
            span: Range::new(Loc(0), Loc(1)).unwrap(),
        };

        let mut select = SelectLocal::new(Loc(5));
        select.visit_decl(&decl);
        assert!(select.selected().is_none());

        let selected = [local];
        let mut calc = CalcDecos::new(selected);

        // A narrow call span exists first.
        calc.visit_term(&MirTerminator::Call {
            destination_local: local,
            fn_span: Range::new(Loc(4), Loc(6)).unwrap(),
        });

        // The super-range call should be ignored (it would only add noise).
        calc.visit_term(&MirTerminator::Call {
            destination_local: local,
            fn_span: Range::new(Loc(0), Loc(10)).unwrap(),
        });

        // And a sub-range should replace the existing one.
        calc.visit_term(&MirTerminator::Call {
            destination_local: local,
            fn_span: Range::new(Loc(4), Loc(5)).unwrap(),
        });

        let decorations = calc.decorations();
        let call_count = decorations
            .iter()
            .filter(|d| matches!(d, Deco::Call { .. }))
            .count();
        assert_eq!(call_count, 1);

        let call_range = decorations.iter().find_map(|d| {
            if let Deco::Call { range, .. } = d {
                Some(*range)
            } else {
                None
            }
        });
        assert_eq!(call_range, Some(Range::new(Loc(4), Loc(5)).unwrap()));
    }

    #[test]
    fn calc_decos_sets_overlapped_on_intersection() {
        let local = FnLocal::new(1, 1);
        let selected = [local];
        let mut calc = CalcDecos::new(selected);

        calc.decorations.push(Deco::ImmBorrow {
            local,
            range: Range::new(Loc(0), Loc(10)).unwrap(),
            hover_text: "immutable borrow".to_string(),
            overlapped: false,
        });
        calc.decorations.push(Deco::Move {
            local,
            range: Range::new(Loc(5), Loc(15)).unwrap(),
            hover_text: "variable moved".to_string(),
            overlapped: false,
        });

        calc.handle_overlapping();

        // Both should have overlapped=true once overlap is detected.
        let overlapped = calc
            .decorations
            .iter()
            .filter(|d| match d {
                Deco::ImmBorrow { overlapped, .. } => *overlapped,
                Deco::Move { overlapped, .. } => *overlapped,
                _ => false,
            })
            .count();
        assert_eq!(overlapped, 2);
    }

    #[test]
    fn calc_decos_does_not_mark_touching_ranges_as_overlapping() {
        let local = FnLocal::new(1, 1);
        let selected = [local];
        let mut calc = CalcDecos::new(selected);

        // Touching at the boundary (until == from) should not count as overlap.
        calc.decorations.push(Deco::ImmBorrow {
            local,
            range: Range::new(Loc(0), Loc(10)).unwrap(),
            hover_text: "immutable borrow".to_string(),
            overlapped: false,
        });
        calc.decorations.push(Deco::Move {
            local,
            range: Range::new(Loc(10), Loc(20)).unwrap(),
            hover_text: "variable moved".to_string(),
            overlapped: false,
        });

        calc.handle_overlapping();

        let any_overlapped = calc.decorations.iter().any(|d| match d {
            Deco::ImmBorrow { overlapped, .. } => *overlapped,
            Deco::Move { overlapped, .. } => *overlapped,
            _ => false,
        });

        assert!(!any_overlapped);
    }
}
