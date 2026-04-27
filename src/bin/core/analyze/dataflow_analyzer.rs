use super::*;
use rustowl::utils;
use std::collections::{HashMap, HashSet};

/// Returns ranges where the given local is certainly initialized.
pub fn get_lives<'tcx>(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.len() == 1 && state.contains(&LocalStateVariant::Initialized)
    })
}

/// Returns ranges where the given local is provably initialized
/// (not moved, dropped, or uninitialized) on every reaching path.
///
/// This will be useful for inspecting variables' resource available range.
pub fn get_maybe_initialized<'tcx>(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.contains(&LocalStateVariant::Initialized)
    })
}

fn check_cfg_analysis_result(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
    eval: impl Fn(&HashSet<LocalStateVariant>) -> bool,
) -> HashMap<LocalId, Vec<Range>> {
    let mut var_initialized: HashMap<LocalId, Vec<Range>> = HashMap::new();
    for (location, states) in cfg_analysis_output {
        for (local, state) in states.iter() {
            if eval(state)
                && let Some(range) = location_ranges.get(location)
            {
                var_initialized.entry(*local).or_default().push(*range);
            }
        }
    }
    var_initialized
        .into_iter()
        .map(|(var, ranges)| (var, utils::eliminated_ranges(ranges)))
        .collect()
}
