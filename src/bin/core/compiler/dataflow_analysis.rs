use super::*;

use rustc_data_structures::indexmap::IndexMap;
use rustc_middle::mir::visit::Visitor;
use std::collections::{HashSet, VecDeque};

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub enum LocalStateVariant {
    Uninitialized = 1,
    Initialized,
    Moved,
    Dropped,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalStates(IndexMap<LocalId, HashSet<LocalStateVariant>>);
impl LocalStates {
    pub fn init_from_body(body: &Body) -> Self {
        let locals = body
            .as_rustc()
            .local_decls
            .iter_enumerated()
            .map(|(local, _)| (AsRustc::from_rustc(local), HashSet::new()))
            .collect();
        Self(locals)
    }
    pub fn join(&mut self, others: &Self) {
        for (key, state) in &mut self.0 {
            if let Some(other) = others.0.get(key) {
                state.extend(other);
            }
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (&LocalId, &HashSet<LocalStateVariant>)> {
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
}
impl<'tcx> Visitor<'tcx> for CfgAnalyzer {
    fn visit_operand(
        &mut self,
        operand: &rustc_middle::mir::Operand<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        let location: Location = AsRustc::from_rustc(location);
        if let rustc_middle::mir::Operand::Move(place) = operand
            && let Some(local_states) = self.states.get_mut(&location)
            && let Some(local) = place.as_local()
            && let Some(state) = local_states
                .0
                .get_mut(&<LocalId as AsRustc>::from_rustc(local))
        {
            if let Some(visited) = self.visited.get(&location)
                && *visited == 0
            {
                state.clear();
            }
            state.insert(LocalStateVariant::Moved);
        }
        self.super_operand(operand, *location.as_rustc());
    }
    fn visit_assign(
        &mut self,
        place: &rustc_middle::mir::Place<'tcx>,
        rvalue: &rustc_middle::mir::Rvalue<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        let location: Location = AsRustc::from_rustc(location);
        if let Some(local_states) = self.states.get_mut(&location)
            && let Some(local) = place.as_local()
            && let Some(state) = local_states
                .0
                .get_mut(&<LocalId as AsRustc>::from_rustc(local))
        {
            if let Some(visited) = self.visited.get(&location)
                && *visited == 0
            {
                state.clear();
            }
            state.insert(LocalStateVariant::Initialized);
        }
        self.super_assign(place, rvalue, *location.as_rustc());
    }
    fn visit_terminator(
        &mut self,
        terminator: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        let location: Location = AsRustc::from_rustc(location);
        if let Some(local_states) = self.states.get_mut(&location) {
            match &terminator.kind {
                rustc_middle::mir::TerminatorKind::Call { destination, .. } => {
                    if let Some(local) = destination.as_local()
                        && let Some(state) = local_states
                            .0
                            .get_mut(&<LocalId as AsRustc>::from_rustc(local))
                    {
                        if let Some(visited) = self.visited.get(&location)
                            && *visited == 0
                        {
                            state.clear();
                        }
                        state.insert(LocalStateVariant::Initialized);
                    }
                }
                rustc_middle::mir::TerminatorKind::Drop { place, .. } => {
                    if let Some(local) = place.as_local()
                        && let Some(state) = local_states
                            .0
                            .get_mut(&<LocalId as AsRustc>::from_rustc(local))
                    {
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
        self.super_terminator(terminator, *location.as_rustc());
    }
}

impl CfgAnalyzer {
    /// Precise lifetime analysis of variables.
    /// Returns
    ///
    /// Note: This code should use our wrapped types, but there are many methods in rustc to use.
    ///       For now we implement this using rustc methods.
    pub fn walk_cfg(body: &Body) -> IndexMap<Location, LocalStates> {
        let mut locals = LocalStates::init_from_body(body);
        let location_local_state: IndexMap<Location, LocalStates> = body
            .as_rustc()
            .basic_blocks
            .iter_enumerated()
            .map(|(block, bb_data)| {
                let locals = locals.clone();
                let statement_len =
                    bb_data.statements.len() + bb_data.terminator.as_ref().map(|_| 1).unwrap_or(0);
                (0..statement_len).map(move |statement_index| {
                    (
                        AsRustc::from_rustc(rustc_middle::mir::Location {
                            block: block.clone(),
                            statement_index,
                        }),
                        locals.clone(),
                    )
                })
            })
            .flatten()
            .collect();

        let block = match body.as_rustc().basic_blocks.iter_enumerated().next() {
            Some((v, _)) => v,
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
        'outer: loop {
            if let Some((block, mut prev_states)) = next_blocks.pop_front()
                && let Some(bb_data) = body.as_rustc().basic_blocks.get(block)
            {
                for (statement_index, statement) in bb_data.statements.iter().enumerate() {
                    let location: Location = AsRustc::from_rustc(rustc_middle::mir::Location {
                        block,
                        statement_index,
                    });

                    let visited = check.visited(&location).map(|v| 0 < *v).unwrap_or(false);
                    if let Some(current_states) = check.states_at(&location) {
                        // Skip check if the location is already visited and the states does not
                        // changed.
                        if visited && *current_states == prev_states {
                            continue 'outer;
                        }

                        current_states.join(&prev_states);
                        check.visit_statement(statement, location.into_rustc());
                    }
                    if let Some(current_states) = check.states_at(&location) {
                        prev_states = current_states.clone();
                    }
                    check.visited(&location).map(|v| *v += 1);
                }
                if let Some(terminator) = &bb_data.terminator {
                    let statement_index = bb_data.statements.len();
                    let location: Location = AsRustc::from_rustc(rustc_middle::mir::Location {
                        block,
                        statement_index,
                    });
                    if let Some(current_states) = check.states_at(&location) {
                        current_states.join(&prev_states);
                        check.visit_terminator(terminator, location.into_rustc());
                    }
                    if let Some(current_states) = check.states_at(&location) {
                        prev_states = current_states.clone();
                    }
                    check.visited(&location).map(|v| *v += 1);

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
