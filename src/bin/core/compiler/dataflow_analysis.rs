use super::*;

use rustc_data_structures::indexmap::{IndexMap, IndexSet};
use rustc_index::bit_set::MixedBitSet;
use rustc_middle::mir::visit::Visitor;
use rustc_mir_dataflow::{
    Analysis, GenKill, MaybeReachable, ResultsVisitor, move_paths::MovePathIndex,
    visit_reachable_results,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Debug)]
pub struct MoveDropTransferFunction<'a>(&'a mut MixedBitSet<rustc_middle::mir::Local>);
impl<'tcx> Visitor<'tcx> for MoveDropTransferFunction<'_> {
    fn visit_operand(
        &mut self,
        operand: &rustc_middle::mir::Operand<'tcx>,
        _location: rustc_middle::mir::Location,
    ) {
        if let rustc_middle::mir::Operand::Move(place) = operand
            && let Some(local) = place.as_local()
        {
            self.0.gen_(local);
        }
    }
    fn visit_terminator(
        &mut self,
        terminator: &rustc_middle::mir::Terminator<'tcx>,
        _location: rustc_middle::mir::Location,
    ) {
        if let rustc_middle::mir::TerminatorKind::Drop { place, .. } = &terminator.kind
            && let Some(local) = place.as_local()
        {
            self.0.gen_(local);
        }
    }
}

pub struct MaybeMovedOrDroppedLocals;
impl MaybeMovedOrDroppedLocals {
    pub fn get_maybe_moved_or_dropped<'tcx>(
        self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
    ) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut visitor = MaybeMovedOrDroppedVisitor::new();
        let results =
            MaybeMovedOrDroppedLocals.iterate_to_fixpoint(tcx.into_rustc(), body.as_rustc(), None);
        visit_reachable_results(body.as_rustc(), &results, &mut visitor);
        visitor.collect()
    }
}
impl<'tcx> Analysis<'tcx> for MaybeMovedOrDroppedLocals {
    type Domain = MixedBitSet<rustc_middle::mir::Local>;
    const NAME: &'static str = "maybe_moved_dropped";

    fn bottom_value(&self, body: &rustc_middle::mir::Body<'tcx>) -> Self::Domain {
        MixedBitSet::new_empty(body.local_decls.len())
    }

    fn initialize_start_block(
        &self,
        body: &rustc_middle::mir::Body<'tcx>,
        state: &mut Self::Domain,
    ) {
    }

    fn apply_primary_statement_effect(
        &self,
        state: &mut Self::Domain,
        statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        MoveDropTransferFunction(state).visit_statement(statement, location);
    }
    fn apply_primary_terminator_effect<'mir>(
        &self,
        state: &mut Self::Domain,
        terminator: &'mir rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) -> rustc_middle::mir::TerminatorEdges<'mir, 'tcx> {
        MoveDropTransferFunction(state).visit_terminator(terminator, location);
        terminator.edges()
    }
}
#[derive(Default)]
struct MaybeMovedOrDroppedVisitor {
    loc_maybe_moved_or_dropped: Vec<(RichLocation, LocalId)>,
}
impl<'a, 'tcx> MaybeMovedOrDroppedVisitor {
    fn new() -> Self {
        Self::default()
    }
    fn push(&mut self, rich_location: RichLocation, local_id: LocalId) {
        self.loc_maybe_moved_or_dropped
            .push((rich_location, local_id));
    }
    fn collect(self) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut result = HashMap::new();
        for (rich_location, local_id) in self.loc_maybe_moved_or_dropped {
            result
                .entry(local_id)
                .or_insert_with(Vec::new)
                .push(rich_location);
        }
        result
    }
}
impl<'tcx> ResultsVisitor<'tcx, MaybeMovedOrDroppedLocals> for MaybeMovedOrDroppedVisitor {
    fn visit_after_primary_statement_effect(
        &mut self,
        _analysis: &MaybeMovedOrDroppedLocals,
        _state: &<MaybeMovedOrDroppedLocals as Analysis>::Domain,
        _statement: &rustc_middle::mir::Statement<'tcx>,
        _location: rustc_middle::mir::Location,
    ) {
    }
    fn visit_after_primary_terminator_effect(
        &mut self,
        _analysis: &MaybeMovedOrDroppedLocals,
        state: &<MaybeMovedOrDroppedLocals as Analysis>::Domain,
        _terminator: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        for local in state.iter() {
            self.push(
                RichLocation::Mid(AsRustc::from_rustc(location)),
                AsRustc::from_rustc(local),
            );
        }
    }
}

impl_as_rustc!(
    MoveData<'tcx>,
    rustc_mir_dataflow::move_paths::MoveData<'tcx>,
);
impl<'tcx> MoveData<'tcx> {
    pub fn gather_moves(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Self {
        Self(<Self as AsRustc>::Rustc::gather_moves(
            body.as_rustc(),
            tcx.into_rustc(),
            |_| true,
        ))
    }
    fn base_local(&self, mpi: MovePathIndex) -> LocalId {
        AsRustc::from_rustc(self.as_rustc().base_local(mpi))
    }
}

impl_as_rustc!(
    MaybeInitializedPlaces<'a, 'tcx>,
    rustc_mir_dataflow::impls::MaybeInitializedPlaces<'a, 'tcx>,
);
impl<'a, 'tcx> MaybeInitializedPlaces<'a, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, body: &'a Body<'tcx>, move_data: &'a MoveData<'tcx>) -> Self {
        Self(<Self as AsRustc>::Rustc::new(
            tcx.into_rustc(),
            body.as_rustc(),
            move_data.as_rustc(),
        ))
    }
    pub fn get_maybe_initialized(
        self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
        move_data: &'a MoveData<'tcx>,
    ) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut visitor = InitializedPlacesVisitor::new(move_data);
        let results =
            self.into_rustc()
                .iterate_to_fixpoint(tcx.into_rustc(), body.as_rustc(), None);
        visit_reachable_results(body.as_rustc(), &results, &mut visitor);
        visitor.collect()
    }
}

struct InitializedPlacesVisitor<'a, 'tcx> {
    move_data: &'a MoveData<'tcx>,
    loc_maybe_initialized: Vec<(RichLocation, LocalId)>,
}
impl<'a, 'tcx> InitializedPlacesVisitor<'a, 'tcx> {
    fn new(move_data: &'a MoveData<'tcx>) -> Self {
        Self {
            move_data,
            loc_maybe_initialized: Default::default(),
        }
    }
    fn push(&mut self, rich_location: RichLocation, move_path_index: MovePathIndex) {
        let local = self.move_data.base_local(move_path_index);
        self.loc_maybe_initialized.push((rich_location, local));
    }
    fn collect(self) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut result = HashMap::new();
        for (rich_location, local_id) in self.loc_maybe_initialized {
            result
                .entry(local_id)
                .or_insert_with(Vec::new)
                .push(rich_location);
        }
        result
    }
}

impl<'a, 'tcx> ResultsVisitor<'tcx, <MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc>
    for InitializedPlacesVisitor<'a, 'tcx>
{
    fn visit_after_early_statement_effect(
        &mut self,
        _analysis: &<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        _statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        if let MaybeReachable::Reachable(mpic) = state {
            for mpi in mpic.iter() {
                self.push(RichLocation::Start(AsRustc::from_rustc(location)), mpi);
            }
        }
    }
    fn visit_after_primary_statement_effect(
        &mut self,
        _analysis: &<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        _statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        if let MaybeReachable::Reachable(mpic) = state {
            for mpi in mpic.iter() {
                self.push(RichLocation::Mid(AsRustc::from_rustc(location)), mpi);
            }
        }
    }
    fn visit_after_early_terminator_effect(
        &mut self,
        _analysis: &<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        _statement: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        if let MaybeReachable::Reachable(mpic) = state {
            for mpi in mpic.iter() {
                self.push(RichLocation::Start(AsRustc::from_rustc(location)), mpi);
            }
        }
    }
    fn visit_after_primary_terminator_effect(
        &mut self,
        _analysis: &<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeInitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        _statement: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        if let MaybeReachable::Reachable(mpic) = state {
            for mpi in mpic.iter() {
                self.push(RichLocation::Mid(AsRustc::from_rustc(location)), mpi);
            }
        }
    }
}

impl_as_rustc!(
    MaybeUninitializedPlaces<'a, 'tcx>,
    rustc_mir_dataflow::impls::MaybeUninitializedPlaces<'a, 'tcx>,
);
impl<'a, 'tcx> MaybeUninitializedPlaces<'a, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, body: &'a Body<'tcx>, move_data: &'a MoveData<'tcx>) -> Self {
        Self(<Self as AsRustc>::Rustc::new(
            tcx.into_rustc(),
            body.as_rustc(),
            move_data.as_rustc(),
        ))
    }
    pub fn get_maybe_uninitialized(
        self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
        move_data: &'a MoveData<'tcx>,
    ) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut visitor = UninitializedPlacesVisitor::new(move_data);
        let results =
            self.into_rustc()
                .iterate_to_fixpoint(tcx.into_rustc(), body.as_rustc(), None);
        visit_reachable_results(body.as_rustc(), &results, &mut visitor);
        visitor.collect()
    }
}

struct UninitializedPlacesVisitor<'a, 'tcx> {
    move_data: &'a MoveData<'tcx>,
    loc_maybe_initialized: Vec<(RichLocation, LocalId)>,
}
impl<'a, 'tcx> UninitializedPlacesVisitor<'a, 'tcx> {
    fn new(move_data: &'a MoveData<'tcx>) -> Self {
        Self {
            move_data,
            loc_maybe_initialized: Default::default(),
        }
    }
    fn push(&mut self, rich_location: RichLocation, move_path_index: MovePathIndex) {
        let local = self.move_data.base_local(move_path_index);
        self.loc_maybe_initialized.push((rich_location, local));
    }
    fn collect(self) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut result = HashMap::new();
        for (rich_location, local_id) in self.loc_maybe_initialized {
            result
                .entry(local_id)
                .or_insert_with(Vec::new)
                .push(rich_location);
        }
        result
    }
}

impl<'a, 'tcx> ResultsVisitor<'tcx, <MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc>
    for UninitializedPlacesVisitor<'a, 'tcx>
{
    fn visit_after_early_statement_effect(
        &mut self,
        _analysis: &<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        _statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        for mpi in state.iter() {
            self.push(RichLocation::Start(AsRustc::from_rustc(location)), mpi);
        }
    }
    fn visit_after_primary_statement_effect(
        &mut self,
        _analysis: &<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        for mpi in state.iter() {
            self.push(RichLocation::Mid(AsRustc::from_rustc(location)), mpi);
        }
    }
    fn visit_after_early_terminator_effect(
        &mut self,
        _analysis: &<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        statement: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        if !matches!(
            statement.kind,
            rustc_middle::mir::TerminatorKind::UnwindResume
        ) {
            for mpi in state.iter() {
                self.push(RichLocation::Start(AsRustc::from_rustc(location)), mpi);
            }
        }
    }
    fn visit_after_primary_terminator_effect(
        &mut self,
        _analysis: &<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc,
        state: &<<MaybeUninitializedPlaces<'a, 'tcx> as AsRustc>::Rustc as Analysis<'tcx>>::Domain,
        statement: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        if !matches!(
            statement.kind,
            rustc_middle::mir::TerminatorKind::UnwindResume
        ) {
            for mpi in state.iter() {
                self.push(RichLocation::Mid(AsRustc::from_rustc(location)), mpi);
            }
        }
    }
}

// Maybe live
impl_as_rustc!(MaybeLiveLocals, rustc_mir_dataflow::impls::MaybeLiveLocals);
impl MaybeLiveLocals {
    pub fn new() -> Self {
        Self(rustc_mir_dataflow::impls::MaybeLiveLocals)
    }
    pub fn get_maybe_lives<'tcx>(
        self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
    ) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut visitor = LiveLocalsVisitor::default();
        let results =
            self.into_rustc()
                .iterate_to_fixpoint(tcx.into_rustc(), body.as_rustc(), None);
        visit_reachable_results(body.as_rustc(), &results, &mut visitor);
        visitor.collect()
    }
}
impl Default for MaybeLiveLocals {
    fn default() -> Self {
        Self::new()
    }
}

/// Visit and collect MIR locals (variables) that live at the location
#[derive(Default, Clone, Debug)]
struct LiveLocalsVisitor {
    loc_maybe_lives: Vec<(RichLocation, HashSet<LocalId>)>,
}
impl LiveLocalsVisitor {
    fn push(&mut self, rich_location: RichLocation, local_ids: impl Iterator<Item = LocalId>) {
        self.loc_maybe_lives
            .push((rich_location, local_ids.collect()));
    }
    /// Transform (Location -> Locals) map into (Local -> Locations) map
    fn collect(self) -> HashMap<LocalId, Vec<RichLocation>> {
        let mut result = HashMap::new();
        for (rich_location, local_ids) in self.loc_maybe_lives {
            let len = local_ids.len();
            for local_id in local_ids {
                result
                    .entry(local_id)
                    .or_insert(Vec::with_capacity(len))
                    .push(rich_location);
            }
        }
        result
    }
}
impl<'tcx> ResultsVisitor<'tcx, <MaybeLiveLocals as AsRustc>::Rustc> for LiveLocalsVisitor {
    fn visit_after_early_statement_effect(
        &mut self,
        _analysis: &<MaybeLiveLocals as AsRustc>::Rustc,
        state: &<<MaybeLiveLocals as AsRustc>::Rustc as Analysis>::Domain,
        _statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        self.push(
            RichLocation::Start(AsRustc::from_rustc(location)),
            state.iter().map(AsRustc::from_rustc),
        );
    }
    fn visit_after_primary_statement_effect(
        &mut self,
        _analysis: &<MaybeLiveLocals as AsRustc>::Rustc,
        state: &<<MaybeLiveLocals as AsRustc>::Rustc as Analysis>::Domain,
        _statement: &rustc_middle::mir::Statement<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        self.push(
            RichLocation::Mid(AsRustc::from_rustc(location)),
            state.iter().map(AsRustc::from_rustc),
        );
    }
    fn visit_after_early_terminator_effect(
        &mut self,
        _analysis: &<MaybeLiveLocals as AsRustc>::Rustc,
        state: &<<MaybeLiveLocals as AsRustc>::Rustc as Analysis>::Domain,
        _terminator: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        self.push(
            RichLocation::Start(AsRustc::from_rustc(location)),
            state.iter().map(AsRustc::from_rustc),
        );
    }
    fn visit_after_primary_terminator_effect(
        &mut self,
        _analysis: &<MaybeLiveLocals as AsRustc>::Rustc,
        state: &<<MaybeLiveLocals as AsRustc>::Rustc as Analysis>::Domain,
        _terminator: &rustc_middle::mir::Terminator<'tcx>,
        location: rustc_middle::mir::Location,
    ) {
        self.push(
            RichLocation::Mid(AsRustc::from_rustc(location)),
            state.iter().map(AsRustc::from_rustc),
        );
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub enum LocalStateVariant {
    Uninitialized = 1,
    Initialized,
    Moved,
    Dropped,
}

#[derive(Clone, Debug)]
pub struct LocalStates(IndexMap<LocalId, HashSet<LocalStateVariant>>);
impl LocalStates {
    pub fn from_body(body: &Body) -> Self {
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

#[derive(Debug)]
pub struct CheckMoveOrDrop {
    states: IndexMap<Location, LocalStates>,
    visited: IndexMap<Location, usize>,
}
impl CheckMoveOrDrop {
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
impl<'tcx> Visitor<'tcx> for CheckMoveOrDrop {
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

/// Precise lifetime analysis of variables.
///
/// Note: This code should use our wrapped types, but there are many methods in rustc to use.
///       For now we implement this using rustc methods.
pub fn walk_cfg(body: &Body) -> IndexMap<Location, LocalStates> {
    let mut locals = LocalStates::from_body(body);
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

    // init local states
    for (_, state) in &mut locals.0 {
        state.insert(LocalStateVariant::Uninitialized);
    }
    // next blocks to check
    let mut next_blocks = VecDeque::new();
    next_blocks.push_back((block, locals));
    let mut check = CheckMoveOrDrop::new(location_local_state);
    // FIXME: Does this loop always stop?
    loop {
        if let Some((block, mut prev_states)) = next_blocks.pop_front()
            && let Some(bb_data) = body.as_rustc().basic_blocks.get(block)
        {
            for (statement_index, statement) in bb_data.statements.iter().enumerate() {
                let location: Location = AsRustc::from_rustc(rustc_middle::mir::Location {
                    block,
                    statement_index,
                });
                if let Some(current_states) = check.states_at(&location) {
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
