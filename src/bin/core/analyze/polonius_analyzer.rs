use rayon::prelude::*;
use rustowl::{models::*, utils};
use std::collections::{HashMap, HashSet};

use super::*;

pub fn get_accurate_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    get_range(
        datafrog
            .var_live_on_entry()
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
) -> (HashMap<LocalId, Vec<Range>>, HashMap<LocalId, Vec<Range>>) {
    let output = datafrog;
    let mut shared_borrows = HashMap::new();
    let mut mutable_borrows = HashMap::new();
    for (location_idx, borrow_idc) in output.loan_live_at().iter() {
        let location = location_table.get_rich_location(location_idx);
        for borrow_idx in borrow_idc {
            match borrow_map.get_from_borrow(borrow_idx) {
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
                    utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &locations)),
                )
            })
            .collect(),
        mutable_borrows
            .into_par_iter()
            .map(|(local, locations)| {
                (
                    local,
                    utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &locations)),
                )
            })
            .collect(),
    )
}

pub fn get_must_live(
    output: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    // obtain a map that region -> region contained locations
    let mut region_locations = HashMap::new();
    for (location_idx, region_idc) in output.origin_live_on_entry().iter() {
        for region_idx in region_idc {
            region_locations
                .entry(*region_idx)
                .or_insert_with(HashSet::new)
                .insert(*location_idx);
        }
    }

    // obtain a map that borrow index -> local
    let mut borrow_local = HashMap::new();
    for (local, borrow_idc) in borrow_map.local_map().iter() {
        for borrow_idx in borrow_idc {
            borrow_local.insert(*borrow_idx, *local);
        }
    }

    // check all regions' subset that must be satisfied
    let mut subsets = HashMap::new();
    for (_, subset) in output.subset().iter() {
        for (sup, subs) in subset.iter() {
            subsets
                .entry(*sup)
                .or_insert_with(HashSet::new)
                .extend(subs.iter().copied());
        }
    }
    // obtain a map that region -> locations
    // a region must contains the locations
    let mut region_must_locations = HashMap::new();
    for (sup, subs) in subsets.iter() {
        for sub in subs {
            if let Some(locs) = region_locations.get(sub) {
                region_must_locations
                    .entry(*sup)
                    .or_insert_with(HashSet::new)
                    .extend(locs.iter().copied());
            }
        }
    }
    // obtain a map that local -> locations
    // a local must lives in the locations
    let mut local_must_locations = HashMap::new();
    for (_location, region_borrows) in output.origin_contains_loan_at().iter() {
        for (region, borrows) in region_borrows.iter() {
            for borrow in borrows {
                if let Some(locs) = region_must_locations.get(region)
                    && let Some(local) = borrow_local.get(borrow)
                {
                    local_must_locations
                        .entry(*local)
                        .or_insert_with(HashSet::new)
                        .extend(locs.iter().copied());
                }
            }
        }
    }

    HashMap::from_iter(local_must_locations.iter().map(|(local, locations)| {
        (
            *local,
            utils::eliminated_ranges(rich_locations_to_ranges(
                basic_blocks,
                &locations
                    .iter()
                    .map(|v| location_table.get_rich_location(v))
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
) -> HashMap<LocalId, Vec<Range>> {
    get_range(
        datafrog
            .var_drop_live_on_entry()
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        basic_blocks,
    )
}

pub fn get_range(
    live_on_entry: impl Iterator<Item = (Point, impl Iterator<Item = LocalId>)>,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<LocalId, Vec<Range>> {
    let mut local_locs = HashMap::new();
    for (point, locals) in live_on_entry {
        let location = location_table.get_rich_location(&point);
        for local in locals {
            local_locs
                .entry(local)
                .or_insert_with(Vec::new)
                .push(location);
        }
    }
    local_locs
        .into_par_iter()
        .map(|(local, locations)| {
            (
                local,
                utils::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &locations)),
            )
        })
        .collect()
}
