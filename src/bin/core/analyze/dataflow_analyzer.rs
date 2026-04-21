use super::*;
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

pub fn get_maybe_uninitialized<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
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
}
