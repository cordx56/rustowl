use super::*;

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
                rich_locations_to_ranges(basic_blocks, &rich_locations),
            )
        })
        .collect()
}
