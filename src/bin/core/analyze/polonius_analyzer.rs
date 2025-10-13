use super::transform::{BorrowData, BorrowMap};
use rayon::prelude::*;
use rustc_borrowck::consumers::{PoloniusLocationTable, PoloniusOutput};
use rustc_index::Idx;
use rustc_middle::mir::Local;
use rustowl::models::{FoldIndexMap as HashMap, FoldIndexSet as HashSet};
use rustowl::{models::*, utils};

pub fn get_accurate_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    get_range(
        datafrog
            .var_live_on_entry
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        basic_blocks,
    )
}

/// returns (shared, mutable)
pub fn get_borrow_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    basic_blocks: &[MirBasicBlock],
) -> (HashMap<Local, Vec<Range>>, HashMap<Local, Vec<Range>>) {
    let output = datafrog;
    let mut shared_borrows = HashMap::default();
    let mut mutable_borrows = HashMap::default();
    for (location_idx, borrow_idc) in output.loan_live_at.iter() {
        let location = location_table.to_rich_location(*location_idx);
        for borrow_idx in borrow_idc {
            match borrow_map.get_from_borrow_index(*borrow_idx) {
                Some((_, BorrowData::Shared { borrowed, .. })) => {
                    shared_borrows
                        .entry(*borrowed)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
                Some((_, BorrowData::Mutable { borrowed, .. })) => {
                    mutable_borrows
                        .entry(*borrowed)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
                _ => {}
            }
        }
    }
    (
        shared_borrows
            .into_par_iter()
            .map(|(local, locations)| {
                (
                    local,
                    utils::eliminated_ranges(super::transform::rich_locations_to_ranges(
                        basic_blocks,
                        &locations,
                    )),
                )
            })
            .collect(),
        mutable_borrows
            .into_par_iter()
            .map(|(local, locations)| {
                (
                    local,
                    utils::eliminated_ranges(super::transform::rich_locations_to_ranges(
                        basic_blocks,
                        &locations,
                    )),
                )
            })
            .collect(),
    )
}

pub fn get_must_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    // obtain a map that region -> region contained locations
    let mut region_locations = HashMap::default();
    for (location_idx, region_idc) in datafrog.origin_live_on_entry.iter() {
        for region_idx in region_idc {
            region_locations
                .entry(*region_idx)
                .or_insert_with(HashSet::default)
                .insert(*location_idx);
        }
    }

    // obtain a map that borrow index -> local
    let mut borrow_local = HashMap::default();
    for (local, borrow_idc) in borrow_map.local_map().iter() {
        for borrow_idx in borrow_idc {
            borrow_local.insert(*borrow_idx, *local);
        }
    }

    // check all regions' subset that must be satisfied
    let mut subsets = HashMap::default();
    for (_, subset) in datafrog.subset.iter() {
        for (sup, subs) in subset.iter() {
            subsets
                .entry(*sup)
                .or_insert_with(HashSet::default)
                .extend(subs.iter().copied());
        }
    }
    // obtain a map that region -> locations
    // a region must contains the locations
    let mut region_must_locations = HashMap::default();
    for (sup, subs) in subsets.iter() {
        for sub in subs {
            if let Some(locs) = region_locations.get(sub) {
                region_must_locations
                    .entry(*sup)
                    .or_insert_with(HashSet::default)
                    .extend(locs.iter().copied());
            }
        }
    }
    // obtain a map that local -> locations
    // a local must lives in the locations
    let mut local_must_locations = HashMap::default();
    for (_location, region_borrows) in datafrog.origin_contains_loan_at.iter() {
        for (region, borrows) in region_borrows.iter() {
            for borrow in borrows {
                if let Some(locs) = region_must_locations.get(region)
                    && let Some(local) = borrow_local.get(borrow)
                {
                    local_must_locations
                        .entry(*local)
                        .or_insert_with(HashSet::default)
                        .extend(locs.iter().copied());
                }
            }
        }
    }

    HashMap::from_iter(local_must_locations.iter().map(|(local, locations)| {
        (
            *local,
            utils::eliminated_ranges(super::transform::rich_locations_to_ranges(
                basic_blocks,
                &locations
                    .iter()
                    .map(|v| location_table.to_rich_location(*v))
                    .collect::<Vec<_>>(),
            )),
        )
    }))
}

/// obtain map from local id to living range
pub fn drop_range(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    get_range(
        datafrog
            .var_drop_live_on_entry
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        basic_blocks,
    )
}

pub fn get_range(
    live_on_entry: impl Iterator<Item = (impl Idx, impl Iterator<Item = impl Idx>)>,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    use rustc_borrowck::consumers::RichLocation;
    use rustc_middle::mir::BasicBlock;

    #[derive(Default)]
    struct LocalLive {
        starts: Vec<(BasicBlock, usize)>,
        mids: Vec<(BasicBlock, usize)>,
    }

    // Collect start/mid locations per local without building an intermediate RichLocation Vec
    let mut locals_live: HashMap<u32, LocalLive> = HashMap::default();
    for (loc_idx, locals) in live_on_entry {
        let rich = location_table.to_rich_location(loc_idx.index().into());
        for local in locals {
            let entry = locals_live
                .entry(local.index().try_into().unwrap())
                .or_insert_with(LocalLive::default);
            match rich {
                RichLocation::Start(l) => entry.starts.push((l.block, l.statement_index)),
                RichLocation::Mid(l) => entry.mids.push((l.block, l.statement_index)),
            }
        }
    }

    fn statement_location_to_range(
        basic_blocks: &[MirBasicBlock],
        block: BasicBlock,
        statement_index: usize,
    ) -> Option<Range> {
        basic_blocks.get(block.index()).and_then(|bb| {
            if statement_index < bb.statements.len() {
                bb.statements.get(statement_index).map(|v| v.range())
            } else {
                bb.terminator.as_ref().map(|v| v.range())
            }
        })
    }

    locals_live
        .into_par_iter()
        .map(|(local_idx, mut live)| {
            super::shared::sort_locs(&mut live.starts);
            super::shared::sort_locs(&mut live.mids);
            let n = live.starts.len().min(live.mids.len());
            if n != live.starts.len() || n != live.mids.len() {
                tracing::debug!(
                    "get_range: starts({}) != mids({}); truncating to {}",
                    live.starts.len(),
                    live.mids.len(),
                    n
                );
            }
            let mut ranges = Vec::with_capacity(n);
            for i in 0..n {
                if let (Some(s), Some(m)) = (
                    statement_location_to_range(basic_blocks, live.starts[i].0, live.starts[i].1),
                    statement_location_to_range(basic_blocks, live.mids[i].0, live.mids[i].1),
                ) && let Some(r) = Range::new(s.from(), m.until())
                {
                    ranges.push(r);
                }
            }
            (local_idx.into(), utils::eliminated_ranges(ranges))
        })
        .collect()
}
