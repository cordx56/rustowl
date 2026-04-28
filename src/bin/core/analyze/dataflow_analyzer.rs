use super::*;
use rustc_data_structures::indexmap::IndexMap;
use rustowl::utils;
use std::collections::{BTreeSet, HashMap, VecDeque};

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
    pub fn new(states: IndexMap<Location, LocalStates>) -> Self {
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

    pub fn visit_statement(&mut self, statement: &MirStatement, location: Location) {
        if let Some(local_states) = self.states.get_mut(&location) {
            match statement {
                MirStatement::Assign {
                    target_local, rval, ..
                } => {
                    if let Some(MirRval::Move { target_local, .. }) = rval
                        && let Some(state) =
                            local_states.0.get_mut(&<LocalId as AsRustc>::from_rustc(
                                rustc_middle::mir::Local::from_u32(target_local.id),
                            ))
                    {
                        if let Some(visited) = self.visited.get(&location)
                            && *visited == 0
                        {
                            state.clear();
                        }
                        state.insert(LocalStateVariant::Moved);
                    }
                    if let Some(state) = local_states.0.get_mut(&<LocalId as AsRustc>::from_rustc(
                        rustc_middle::mir::Local::from_u32(target_local.id),
                    )) {
                        if let Some(visited) = self.visited.get(&location)
                            && *visited == 0
                        {
                            state.clear();
                        }
                        state.insert(LocalStateVariant::Initialized);
                    }
                }
                MirStatement::StorageDead { target_local, .. } => {
                    if let Some(state) = local_states.0.get_mut(&<LocalId as AsRustc>::from_rustc(
                        rustc_middle::mir::Local::from_u32(target_local.id),
                    )) {
                        if let Some(visited) = self.visited.get(&location)
                            && *visited == 0
                        {
                            state.clear();
                        }
                        state.insert(LocalStateVariant::Uninitialized);
                    }
                }
                _ => {}
            }
        }
    }
    pub fn visit_terminator(&mut self, terminator: &MirTerminator, location: Location) {
        if let Some(local_states) = self.states.get_mut(&location) {
            match &terminator {
                MirTerminator::Call {
                    destination_local, ..
                } => {
                    if let Some(state) = local_states.0.get_mut(&<LocalId as AsRustc>::from_rustc(
                        rustc_middle::mir::Local::from_u32(destination_local.id),
                    )) {
                        if let Some(visited) = self.visited.get(&location)
                            && *visited == 0
                        {
                            state.clear();
                        }
                        state.insert(LocalStateVariant::Initialized);
                    }
                }
                MirTerminator::Drop { local, .. } => {
                    if let Some(state) = local_states.0.get_mut(&<LocalId as AsRustc>::from_rustc(
                        rustc_middle::mir::Local::from_u32(local.id),
                    )) {
                        if let Some(visited) = self.visited.get(&location)
                            && *visited == 0
                        {
                            state.clear();
                        }
                        state.insert(LocalStateVariant::Dropped);
                    }
                }
                _ => {}
            }
        }
    }

    /// Precise lifetime analysis of variables.
    ///
    /// Note: This code should use our wrapped types, but there are many methods in rustc to use.
    ///       For now we implement this using rustc methods.
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
        let mut check = Self::new(location_local_state);
        // FIXME: Does this loop always stop?
        'outer: for _ in 0..(5 * basic_blocks.len()) {
            if let Some((block, mut prev_states)) = next_blocks.pop_front()
                && let Some(bb_data) = basic_blocks.get(&block)
            {
                for (statement_index, statement) in bb_data.statements.iter().enumerate() {
                    let location: Location = AsRustc::from_rustc(rustc_middle::mir::Location {
                        block: rustc_middle::mir::BasicBlock::from_usize(block.0),
                        statement_index,
                    });

                    let visited = check.visited(&location).map(|v| *v).unwrap_or(0);
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

/// Returns ranges where the given local is certainly initialized.
pub fn get_lives(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.len() == 1 && state.contains(&LocalStateVariant::Initialized)
    })
}

/// Returns ranges where the given local is provably initialized
/// (not moved, dropped, or uninitialized) on every reaching path.
///
/// This will be useful for inspecting variables' resource available range.
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
