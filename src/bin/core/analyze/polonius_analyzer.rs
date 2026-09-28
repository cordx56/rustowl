use rayon::prelude::*;
use rustowl::{models::*, utils};
use std::collections::{HashMap, HashSet};

use super::*;

pub fn get_accurate_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    get_range(
        datafrog
            .var_live_on_entry()
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        location_ranges,
    )
}

/// returns (shared, mutable)
pub fn get_borrow_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    location_ranges: &LocationRanges,
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
                    utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, &locations)),
                )
            })
            .collect(),
        mutable_borrows
            .into_par_iter()
            .map(|(local, locations)| {
                (
                    local,
                    utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, &locations)),
                )
            })
            .collect(),
    )
}

/// obtain a map that region -> locations where a variable
/// whose type contains the region is used or dropped
fn region_access_locations(input: &PoloniusInput) -> HashMap<Region, HashSet<Point>> {
    let mut result = HashMap::new();
    for (var_accessed_at, var_derefs_origin) in [
        (input.var_used_at(), input.use_of_var_derefs_origin()),
        (input.var_dropped_at(), input.drop_of_var_derefs_origin()),
    ] {
        let mut local_regions = HashMap::new();
        for (local, region) in var_derefs_origin {
            local_regions
                .entry(local)
                .or_insert_with(Vec::new)
                .push(region);
        }
        for (local, location_idx) in var_accessed_at {
            for region in local_regions.get(&local).into_iter().flatten() {
                result
                    .entry(*region)
                    .or_insert_with(HashSet::new)
                    .insert(location_idx);
            }
        }
    }
    result
}

pub fn get_must_live(
    input: &PoloniusInput,
    output: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    // obtain a map that borrow index -> local
    let mut borrow_local = HashMap::new();
    for (local, borrow_idc) in borrow_map.local_map().iter() {
        for borrow_idx in borrow_idc {
            borrow_local.insert(*borrow_idx, *local);
        }
    }

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

    // obtain a map that region -> locations where region must be live
    // For subset relation sup >= sub at point p:
    // - if sup is live at p, sup itself must be live at p (for borrows contained in sup)
    // - if sup is live at p, sub must also be live at p (for borrows contained in sub)
    // IMPORTANT: subset relations only apply from the point where they are established
    let mut region_must_locations = HashMap::new();
    for (location_idx, subset) in output.subset().iter() {
        for (sup, subs) in subset.iter() {
            // If sup region is live at this point
            if region_locations
                .get(sup)
                .is_some_and(|locs| locs.contains(location_idx))
            {
                // sup is must_live at this point (for borrows contained in sup)
                region_must_locations
                    .entry(*sup)
                    .or_insert_with(HashSet::new)
                    .insert(*location_idx);
                // sub regions are also must_live at this point
                for sub in subs {
                    region_must_locations
                        .entry(*sub)
                        .or_insert_with(HashSet::new)
                        .insert(*location_idx);
                }
            }
        }
    }
    // a region must also be live where a variable whose type contains it is used or dropped;
    // the borrows in the region may be dereferenced there
    for (region, locations) in region_access_locations(input) {
        region_must_locations
            .entry(region)
            .or_insert_with(HashSet::new)
            .extend(locations);
    }

    // obtain a map that local -> locations
    let mut local_must_locations = HashMap::new();
    for (location_idx, region_borrows) in output.origin_contains_loan_at().iter() {
        for (region, borrows) in region_borrows.iter() {
            if region_must_locations
                .get(region)
                .is_some_and(|locs| locs.contains(location_idx))
            {
                for local in borrows.iter().filter_map(|borrow| borrow_local.get(borrow)) {
                    local_must_locations
                        .entry(*local)
                        .or_insert_with(HashSet::new)
                        .insert(*location_idx);
                }
            }
        }
    }

    HashMap::from_iter(local_must_locations.iter().map(|(local, locations)| {
        (
            *local,
            utils::eliminated_ranges(rich_locations_to_ranges(
                location_ranges,
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
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    get_range(
        datafrog
            .var_drop_live_on_entry()
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        location_ranges,
    )
}

pub fn reference_local_live_range(
    output: &PoloniusOutput,
    region_vids: impl Iterator<Item = (LocalId, RegionVid)>,
    location_table: &PoloniusLocationTable,
    location_ranges: &LocationRanges,
) -> IndexMap<LocalId, Vec<Range>> {
    region_vids
        .map(|(local, vid)| {
            let locations: Vec<_> = output
                .origin_live_on_entry()
                .iter()
                .filter_map(|(p, r)| {
                    let polonius_vid =
                        rustc_borrowck::consumers::PoloniusRegionVid::from(vid.into_rustc());
                    if r.iter()
                        .map(|v| v.into_rustc())
                        .find(|v| *v == polonius_vid)
                        .is_some()
                    {
                        Some(location_table.get_rich_location(p))
                    } else {
                        None
                    }
                })
                .collect();
            (
                local,
                utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, &locations)),
            )
        })
        .collect()
}

pub fn get_range(
    live_on_entry: impl Iterator<Item = (Point, impl Iterator<Item = LocalId>)>,
    location_table: &PoloniusLocationTable,
    location_ranges: &LocationRanges,
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
                utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, &locations)),
            )
        })
        .collect()
}
