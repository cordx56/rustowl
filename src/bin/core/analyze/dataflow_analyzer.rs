use super::*;
use rustc_span::Pos;
use rustowl::utils;

pub fn get_maybe_lives<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    MaybeLiveLocals::new()
        .get_maybe_lives(tcx, body)
        .into_iter()
        .map(|(local_id, rich_locations)| {
            (
                local_id,
                utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &rich_locations)),
            )
        })
        .collect()
}

/*
pub fn get_maybe_initialized<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    let move_data = MoveData::gather_moves(tcx, body);
    MaybeInitializedPlaces::new(tcx, body, &move_data)
        .get_maybe_initialized(tcx, body, &move_data)
        .into_iter()
        .map(|(local_id, rich_locations)| {
            (
                local_id,
                utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &rich_locations)),
            )
        })
        .collect()
}
*/

pub fn get_maybe_uninitialized<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    MaybeMovedOrDroppedLocals
        .get_maybe_moved_or_dropped(tcx, body)
        .into_iter()
        .map(|(local_id, rich_locations)| {
            (
                local_id,
                utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &rich_locations)),
            )
        })
        .collect()
    /*
    let move_data = MoveData::gather_moves(tcx, body);
    MaybeUninitializedPlaces::new(tcx, body, &move_data)
        .get_maybe_uninitialized(tcx, body, &move_data)
        .into_iter()
        .map(|(local_id, rich_locations)| {
            (
                local_id,
                utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &rich_locations)),
            )
        })
        .collect()
    */
}

pub fn get_maybe_initialized<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    //let mut result = HashMap::new();
    let mut var_initialized = HashMap::new();
    for (location, states) in walk_cfg(body) {
        for (local, state) in states.iter() {
            if state.len() == 1 && state.contains(&LocalStateVariant::Initialized) {
                var_initialized
                    .entry(*local)
                    .or_insert_with(Vec::new)
                    .push(RichLocation::Mid(location));
                /*
                let range = rich_locations_to_ranges(basic_blocks, &[RichLocation::Mid(location)]);
                let span = body.as_rustc().source_info(location.into_rustc()).span;
                let loc_low = Loc::new()
                if let Some(range) = Range::new(span.lo().to_u32().into(), span.hi().to_u32().into()) {
                    eprintln!("{range:?}");
                    if range.size() < 180 {
                result
                    .entry(*local)
                    .or_insert_with(Vec::new)
                    .push(range);
                    }
                }
                */
            }
        }
    }
    var_initialized
        .iter()
        .map(|(var, locs)| (*var, rich_locations_to_ranges(basic_blocks, locs)))
        .collect()
    //result
}
