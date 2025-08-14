use rayon::prelude::*;
use rustc_borrowck::consumers::{BorrowSet, PoloniusLocationTable, PoloniusOutput};
use rustc_index::Idx;
use rustc_middle::mir::Local;
use rustowl::{models::*, utils};
use std::collections::{BTreeSet, HashMap};

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
    borrow_set: &BorrowSet<'_>,
    basic_blocks: &[MirBasicBlock],
) -> (HashMap<Local, Vec<Range>>, HashMap<Local, Vec<Range>>) {
    let output = datafrog;
    let mut shared_borrows = HashMap::new();
    let mut mutable_borrows = HashMap::new();
    for (location_idx, borrow_idc) in output.loan_live_at.iter() {
        let location = location_table.to_rich_location(*location_idx);
        for borrow_idx in borrow_idc {
            let borrow_data = &borrow_set[*borrow_idx];
            let local = borrow_data.borrowed_place().local;
            if borrow_data.kind().mutability().is_mut() {
                mutable_borrows
                    .entry(local)
                    .or_insert_with(Vec::new)
                    .push(location);
            } else {
                shared_borrows
                    .entry(local)
                    .or_insert_with(Vec::new)
                    .push(location);
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
    insensitive: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_set: &BorrowSet<'_>,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    let mut region_locations = HashMap::new();
    for (location_idx, region_idc) in insensitive.origin_live_on_entry.iter() {
        let location = location_table.to_rich_location(*location_idx);
        for region_idx in region_idc {
            region_locations
                .entry(*region_idx)
                .or_insert_with(Vec::new)
                .push(location);
        }
    }

    // local -> all borrows on that local
    let mut borrow_locals = HashMap::new();
    for (local, borrow_idc) in borrow_set.local_map().iter() {
        for borrow_idx in borrow_idc {
            borrow_locals.insert(*borrow_idx, *local);
        }
    }

    // compute regions where the local must be live
    let mut local_must_regions: HashMap<Local, BTreeSet<super::Region>> = HashMap::new();
    for (region_idx, borrow_idc) in insensitive.origin_contains_loan_anywhere.iter() {
        for borrow_idx in borrow_idc {
            if let Some(local) = borrow_locals.get(borrow_idx) {
                local_must_regions
                    .entry(*local)
                    .or_default()
                    .insert(*region_idx);
            }
        }
    }

    HashMap::from_iter(local_must_regions.iter().map(|(local, regions)| {
        (
            *local,
            super::erase_superset(
                super::transform::rich_locations_to_ranges(
                    basic_blocks,
                    &regions
                        .par_iter()
                        .filter_map(|v| region_locations.get(v).cloned())
                        .flatten()
                        .collect::<Vec<_>>(),
                ),
                false,
            ),
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
    let mut local_locs = HashMap::new();
    for (loc_idx, locals) in live_on_entry {
        let location = location_table.to_rich_location(loc_idx.index().into());
        for local in locals {
            local_locs
                .entry(local.index())
                .or_insert_with(Vec::new)
                .push(location);
        }
    }
    local_locs
        .into_par_iter()
        .map(|(local, locations)| {
            (
                local.into(),
                utils::eliminated_ranges(super::transform::rich_locations_to_ranges(
                    basic_blocks,
                    &locations,
                )),
            )
        })
        .collect()
}
