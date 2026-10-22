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
            .iter()
            .map(|(local, locations)| {
                (
                    *local,
                    utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, locations)),
                )
            })
            .collect(),
        mutable_borrows
            .iter()
            .map(|(local, locations)| {
                (
                    *local,
                    utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, locations)),
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

#[inline]
fn location_of(location_table: &PoloniusLocationTable, point: Point) -> Location {
    match location_table.get_rich_location(&point) {
        RichLocation::Start(location) | RichLocation::Mid(location) => location,
    }
}

/// obtain a map that local -> locations where a borrow of the local is used
/// or dropped after the borrowed value has ended (the lifetime deficit)
///
/// `value_ends` contains the locations that end the value of a local
/// (see [`dataflow_analyzer::collect_value_ends`]).
pub fn get_deficit(
    input: &PoloniusInput,
    output: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    value_ends: &HashSet<(Location, LocalId)>,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    // obtain a map that borrow index -> local
    let mut borrow_local = HashMap::new();
    for (local, borrow_idc) in borrow_map.local_map().iter() {
        for borrow_idx in borrow_idc {
            borrow_local.insert(*borrow_idx, *local);
        }
    }

    // `origin_live` is a set of (p, region) such that the region is live on entry to p
    let origin_live: HashSet<(Point, Region)> = output
        .origin_live_on_entry()
        .into_iter()
        .flat_map(|(point, regions)| regions.into_iter().map(move |region| (point, region)))
        .collect();

    // obtain a map that point -> next points of CFG edges
    let mut successors = HashMap::new();
    for (from, to) in input.cfg_edge() {
        successors.entry(from).or_insert_with(Vec::new).push(to);
    }

    // `loan_ends` is a set of (p, borrow) such that the borrow is invalidated at p
    // by an access that ends the value of the borrowed local (move, drop or StorageDead)
    let loan_ends: HashSet<(Point, Borrow)> = input
        .loan_invalidated_at()
        .into_iter()
        .filter(|(point, borrow)| {
            borrow_local.get(borrow).is_some_and(|local| {
                value_ends.contains(&(location_of(location_table, *point), *local))
            })
        })
        .collect();

    // `dead` is a set of (p, region, borrow) such that the region may contain the borrow
    // at p after the value of the borrowed local has ended
    let mut dead = HashSet::new();
    let mut stack_working = Vec::new();
    // seed `dead` with (p, region, borrow) such that the borrow ends at p
    // while a live region contains it; a dead region has no later use
    for (point, region_borrows) in output.origin_contains_loan_at() {
        for (region, borrows) in region_borrows {
            if !origin_live.contains(&(point, region)) {
                continue;
            }
            for borrow in borrows {
                if loan_ends.contains(&(point, borrow)) && dead.insert((point, region, borrow)) {
                    stack_working.push((point, region, borrow));
                }
            }
        }
    }

    // `subset` represents `(region1 <: region2) @ p` as `p -> (region1 -> {region2})`
    // where borrows in region1 flow into region2 at p
    let subset = output.subset();
    // propagate `dead` in the same way as `origin_contains_loan_on_entry` of Polonius,
    // but ignoring `loan_killed_at`; a kill means that the borrowed local gets a new value,
    // while the borrow still refers to the ended one
    while let Some((point, region, borrow)) = stack_working.pop() {
        // `to_regions` is a set of (p, region2) such that `(region <: region2) @ p`
        let to_regions = subset
            .get(&point)
            .and_then(|regions| regions.get(&region))
            .into_iter()
            .flatten()
            .map(|to| (point, *to));
        // `to_points` is a set of (q, region) such that q is a next point of p
        // and the region is live on entry to q
        let to_points = successors
            .get(&point)
            .into_iter()
            .flatten()
            .filter(|next| origin_live.contains(&(**next, region)))
            .map(|next| (*next, region));
        for (next_point, next_region) in to_regions.chain(to_points) {
            if dead.insert((next_point, next_region, borrow)) {
                stack_working.push((next_point, next_region, borrow));
            }
        }
    }

    // obtain a map that region -> points where a variable whose type contains
    // the region is used or dropped
    let access = region_access_locations(input);
    // obtain a map that local -> points where a region that contains a
    // borrow of the local after its end is used or dropped;
    // at the other points in `dead`, the region only holds the borrow
    let mut local_deficit_locations = HashMap::new();
    for (point, region, borrow) in dead {
        if access
            .get(&region)
            .is_some_and(|points| points.contains(&point))
            && let Some(local) = borrow_local.get(&borrow)
        {
            local_deficit_locations
                .entry(*local)
                .or_insert_with(HashSet::new)
                .insert(point);
        }
    }

    // convert the points into source ranges for each local
    HashMap::from_iter(local_deficit_locations.iter().map(|(local, locations)| {
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
    let origin_live_on_entry = output.origin_live_on_entry();
    region_vids
        .map(|(local, vid)| {
            let polonius_vid = rustc_borrowck::consumers::PoloniusRegionVid::from(vid.into_rustc());
            let locations: Vec<_> = origin_live_on_entry
                .iter()
                .filter_map(|(p, r)| {
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
        .iter()
        .map(|(local, locations)| {
            (
                *local,
                utils::eliminated_ranges(rich_locations_to_ranges(location_ranges, locations)),
            )
        })
        .collect()
}
