//! CFG-based liveness analysis.
//!
//! Walks the MIR control-flow graph and tracks, for each [`Location`], the
//! set of states that each local can be in along any path that reaches it.
//! The result complements Polonius' `var_live_on_entry` by distinguishing
//! "provably initialized" from "initialized on some paths only".
//!
//! The state lattice for a single local is the powerset of
//! [`LocalStateVariant`]; values flow forward and meet at CFG joins via
//! set union ([`LocalStates::join`]). From the per-location state set we
//! derive two range collections:
//!
//! - [`get_definitely_lives`] -- locations where the state is exactly
//!   `{Initialized}`. These are the ranges shown as the green
//!   "definitely live" decoration.
//! - [`get_maybe_initialized`] -- locations where `Initialized` is in the
//!   state set together with at least one of `Moved`, `Dropped`, or
//!   `Uninitialized`. These mark places where ownership depends on which
//!   path was taken (e.g. a conditional `drop`), and are useful when
//!   auditing resource management.

use super::*;
use indexmap::IndexMap;
use rustowl::utils;
use std::collections::{BTreeSet, HashMap, VecDeque};

/// One element of the per-local state lattice.
///
/// State transitions performed by [`CfgAnalyzer::visit_statement`] and
/// [`CfgAnalyzer::visit_terminator`]:
///
/// - `Assign` to a local sets it to `Initialized`. If the rvalue is a
///   `Move`, the source local is set to `Moved` first.
/// - `StorageDead` sets the local to `Uninitialized`.
/// - A `Call` terminator sets each `Move` argument to `Moved` and the
///   destination local to `Initialized`.
/// - A `Drop` terminator removes `Initialized` and adds `Dropped`. Other
///   variants (e.g. an earlier `Moved`) survive so that joins keep
///   reflecting all paths reaching the location.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum LocalStateVariant {
    Uninitialized = 1,
    Initialized,
    Moved,
    Dropped,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalStates(IndexMap<LocalId, BTreeSet<LocalStateVariant>>);
impl LocalStates {
    pub fn init_from_locals(locals: impl Iterator<Item = LocalId>) -> Self {
        Self(locals.map(|v| (v, BTreeSet::new())).collect())
    }
    /// Meet operation for the lattice: per-local set union with `others`.
    /// Used at CFG join points so that a local's state at a successor is
    /// the union of the states reaching it from each predecessor.
    pub fn join(&mut self, others: &Self) {
        for (key, state) in &mut self.0 {
            if let Some(other) = others.0.get(key) {
                state.extend(other);
            }
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (&LocalId, &BTreeSet<LocalStateVariant>)> {
        self.0.iter()
    }
}

pub type CfgAnalysisOutput = IndexMap<Location, LocalStates>;

/// Walks MIR [`Body`]'s CFG and collects [`LocalId`]'s state at each [`Location`].
#[derive(Debug)]
pub struct CfgAnalyzer {
    states: IndexMap<Location, LocalStates>,
    visited: IndexMap<Location, usize>,
}
impl CfgAnalyzer {
    fn init(states: IndexMap<Location, LocalStates>) -> Self {
        let visited = states.iter().map(|(location, _)| (*location, 0)).collect();
        Self { states, visited }
    }
    pub fn states_at(&mut self, location: &Location) -> Option<&mut LocalStates> {
        self.states.get_mut(location)
    }
    pub fn visited(&mut self, location: &Location) -> Option<&mut usize> {
        self.visited.get_mut(location)
    }
    pub fn finish(self) -> IndexMap<Location, LocalStates> {
        self.states
    }

    pub fn visit_operand(&mut self, operand: &MirOperand, location: Location) {
        if let MirOperand::Move { place, .. } = operand
            && let Some(local_states) = self.states.get_mut(&location)
            && let Some(state) =
                local_states
                    .0
                    .get_mut(&LocalId::from_rustc(rustc_middle::mir::Local::from_u32(
                        place.local.id,
                    )))
        {
            state.clear();
            state.insert(LocalStateVariant::Moved);
        }
    }
    pub fn visit_rval(&mut self, rval: &MirRval, location: Location) {
        match rval {
            MirRval::Use { operand }
            | MirRval::Repeat { operand }
            | MirRval::Cast { operand }
            | MirRval::UnaryOp { operand } => {
                self.visit_operand(operand, location);
            }
            MirRval::BinaryOp { left, right } => {
                self.visit_operand(left, location);
                self.visit_operand(right, location);
            }
            MirRval::Aggregate { fields } => {
                for field in fields {
                    self.visit_operand(field, location);
                }
            }
            MirRval::Ref { .. } | MirRval::Other => {}
        }
    }
    pub fn visit_statement(&mut self, statement: &MirStatement, location: Location) {
        match &statement.kind {
            MirStatementKind::Assign { place, rval, .. } => {
                self.visit_rval(rval, location);
                if let Some(local_states) = self.states.get_mut(&location)
                    && let Some(state) = local_states.0.get_mut(&LocalId::from_rustc(
                        rustc_middle::mir::Local::from_u32(place.local.id),
                    )) {
                        state.clear();
                        state.insert(LocalStateVariant::Initialized);
                    }
            }
            MirStatementKind::StorageDead { local } => {
                if let Some(local_states) = self.states.get_mut(&location)
                    && let Some(state) = local_states.0.get_mut(&LocalId::from_rustc(
                        rustc_middle::mir::Local::from_u32(local.id),
                    )) {
                        state.clear();
                        state.insert(LocalStateVariant::Uninitialized);
                    }
            }
            _ => {}
        }
    }
    pub fn visit_terminator(&mut self, terminator: &MirTerminator, location: Location) {
        match &terminator.kind {
            MirTerminatorKind::SwitchInt { discr, .. } => {
                self.visit_operand(discr, location);
            }
            MirTerminatorKind::Drop { place, .. } => {
                if let Some(local_states) = self.states.get_mut(&location)
                    && let Some(state) = local_states.0.get_mut(&LocalId::from_rustc(
                        rustc_middle::mir::Local::from_u32(place.local.id),
                    ))
                {
                    state.remove(&LocalStateVariant::Initialized);
                    state.insert(LocalStateVariant::Dropped);
                }
            }
            MirTerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } => {
                self.visit_operand(func, location);
                for arg in args {
                    self.visit_operand(arg, location);
                }
                if let Some(local_states) = self.states.get_mut(&location)
                    && let Some(state) = local_states.0.get_mut(&LocalId::from_rustc(
                        rustc_middle::mir::Local::from_u32(destination.local.id),
                    ))
                {
                    state.clear();
                    state.insert(LocalStateVariant::Initialized);
                }
            }
            MirTerminatorKind::TailCall { func, args, .. } => {
                self.visit_operand(func, location);
                for arg in args {
                    self.visit_operand(arg, location);
                }
            }
            MirTerminatorKind::Assert { cond, .. } => {
                self.visit_operand(cond, location);
            }

            _ => {}
        }
    }

    /// Forward dataflow over the CFG, returning the per-local state set at
    /// each [`Location`].
    ///
    /// Starts at the entry block with every local marked `Uninitialized`
    /// and walks blocks in BFS order, joining the carried state into each
    /// location and re-enqueueing successors when state changes.
    ///
    /// Termination is enforced by two cutoffs rather than a proof of
    /// monotone convergence: each location may be visited at most 10 times
    /// (per-location circuit breaker), and the outer queue runs for at
    /// most `10 * basic_blocks.len()` iterations. In practice the lattice
    /// is small (4 variants per local) so a fixpoint is reached well
    /// before either cutoff; the cutoffs only protect against pathological
    /// inputs (e.g. unreachable cycles introduced by ill-formed MIR).
    pub fn walk_cfg(
        basic_blocks: &IndexMap<BasicBlockId, MirBasicBlock>,
        locals: &BTreeMap<LocalId, String>,
    ) -> IndexMap<Location, LocalStates> {
        let mut locals = LocalStates::init_from_locals(locals.keys().copied());
        let location_local_state: IndexMap<Location, LocalStates> = basic_blocks
            .iter()
            .flat_map(|(block, bb_data)| {
                let locals = locals.clone();
                let statement_len =
                    bb_data.statements.len() + bb_data.terminator.as_ref().map(|_| 1).unwrap_or(0);
                (0..statement_len).map(move |statement_index| {
                    (
                        AsRustc::from_rustc(rustc_middle::mir::Location {
                            block: rustc_middle::mir::BasicBlock::from_usize(block.0),
                            statement_index,
                        }),
                        locals.clone(),
                    )
                })
            })
            .collect();

        let block = match basic_blocks.first() {
            Some((v, _)) => *v,
            None => return location_local_state,
        };

        // init local states with uninitialized state
        for (_, state) in &mut locals.0 {
            state.insert(LocalStateVariant::Uninitialized);
        }
        // next blocks to check
        let mut next_blocks = VecDeque::new();
        // use the last states at the previous block when start walking the new block.
        next_blocks.push_back((block, locals));
        let mut check = Self::init(location_local_state);
        // Termination: bounded by per-location visit cap (see below) and
        // the outer iteration cap. See the doc on `walk_cfg` for rationale.
        'outer: for _ in 0..(10 * basic_blocks.len()) {
            if let Some((block, mut prev_states)) = next_blocks.pop_front()
                && let Some(bb_data) = basic_blocks.get(&block)
            {
                for (statement_index, statement) in bb_data.statements.iter().enumerate() {
                    let location: Location = AsRustc::from_rustc(rustc_middle::mir::Location {
                        block: rustc_middle::mir::BasicBlock::from_usize(block.0),
                        statement_index,
                    });

                    let visited = check.visited(&location).map(|v| *v).unwrap_or(0);
                    // Skip check if same location is visited many times (circuit breaker)
                    if 10 <= visited {
                        continue 'outer;
                    }
                    if let Some(current_states) = check.states_at(&location) {
                        // Skip check if the location is already visited and the states does not
                        // changed.
                        if 0 < visited && *current_states == prev_states {
                            continue 'outer;
                        }

                        current_states.join(&prev_states);
                        check.visit_statement(statement, location);
                    }
                    if let Some(current_states) = check.states_at(&location) {
                        prev_states = current_states.clone();
                    }
                    if let Some(v) = check.visited(&location) {
                        *v += 1;
                    }
                }
                if let Some(terminator) = &bb_data.terminator {
                    let statement_index = bb_data.statements.len();
                    let location: Location = AsRustc::from_rustc(rustc_middle::mir::Location {
                        block: rustc_middle::mir::BasicBlock::from_usize(block.0),
                        statement_index,
                    });
                    if let Some(current_states) = check.states_at(&location) {
                        current_states.join(&prev_states);
                        check.visit_terminator(terminator, location);
                    }
                    if let Some(current_states) = check.states_at(&location) {
                        prev_states = current_states.clone();
                    }
                    if let Some(v) = check.visited(&location) {
                        *v += 1;
                    }

                    for successor in terminator.successors() {
                        next_blocks.push_back((successor, prev_states.clone()));
                    }
                }
            } else {
                break;
            }
        }
        check.finish()
    }
}

/// Source ranges where each local is definitely initialized.
///
/// A location qualifies when the per-local state set is exactly
/// `{Initialized}`, i.e. every path reaching the location leaves the local
/// in the initialized state. Surfaced as the green "definitely live"
/// decoration.
pub fn get_definitely_lives(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.len() == 1 && state.contains(&LocalStateVariant::Initialized)
    })
}

/// Source ranges where each local is initialized on at least one path,
/// possibly together with `Moved`, `Dropped`, or `Uninitialized` on others.
///
/// This is a strict superset of [`get_definitely_lives`]; the difference
/// (a "maybe live" range) marks code where ownership is conditional on
/// control flow, which is the interesting case for auditing resource
/// cleanup such as `Drop` impls, file handles, or locks.
pub fn get_maybe_initialized(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.contains(&LocalStateVariant::Initialized)
    })
}

fn check_cfg_analysis_result(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
    eval: impl Fn(&BTreeSet<LocalStateVariant>) -> bool,
) -> HashMap<LocalId, Vec<Range>> {
    let mut var_initialized: HashMap<LocalId, Vec<Range>> = HashMap::new();
    for (location, states) in cfg_analysis_output {
        for (local, state) in states.iter() {
            if eval(state)
                && let Some(range) = location_ranges.get(location)
            {
                var_initialized.entry(*local).or_default().push(*range);
            }
        }
    }
    var_initialized
        .into_iter()
        .map(|(var, ranges)| (var, utils::eliminated_ranges(ranges)))
        .collect()
}
