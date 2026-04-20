use super::*;

use rustc_mir_dataflow::{Analysis, ResultsVisitor, visit_reachable_results};
use std::collections::{HashMap, HashSet};

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
