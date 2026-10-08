//! CFG-based liveness analysis.
//!
//! Walks the MIR control-flow graph and tracks, for each [`Location`], the
//! set of states that each place can be in along any path that reaches it.
//! The result complements Polonius' `var_live_on_entry` by distinguishing
//! "provably initialized" from "initialized on some paths only".
//!
//! The state lattice for a single place is the power-set of
//! [`LocalStateVariant`]; values flow forward and meet at CFG joins via
//! set union ([`States::join`]) up to the least fixed point.
//! The state of a local is that of the whole local, combined from the
//! states of the local and its fragments ([`StateBitSet::whole`]).
//! From the per-location state set we derive two range collections:
//!
//! - [`get_definitely_lives`] -- locations where the state is exactly
//!   `{Initialized}`. These are the ranges shown as the green
//!   "definitely live" decoration.
//! - [`get_maybe_initialized`] -- locations where `Initialized` is in the
//!   state set together with at least one of `Moved`, `Dropped`, or
//!   `Uninitialized`. These mark places where ownership depends on which
//!   path was taken (e.g. a conditional `drop`), and are useful when
//!   auditing resource management.

use super::*;
use indexmap::IndexMap;
use rustowl::utils;
use std::collections::{HashMap, HashSet, VecDeque};

mod effect;
mod places;
mod state;

pub use effect::*;
use places::*;
use state::*;

/// Whether `fragment` is a fragment of `place`: a proper sub-place within the
/// storage of `place`, that is, not behind a dereference.
fn is_fragment(place: &MirPlace, fragment: &MirPlace) -> bool {
    place.local == fragment.local
        && place.projection.len() < fragment.projection.len()
        && fragment.projection.starts_with(&place.projection)
        && !fragment.projection[place.projection.len()..]
            .iter()
            .any(|elem| matches!(elem, MirProjectionElem::Deref))
}

/// Collect the locations that end the value of a local: a move out of it
/// or out of a place based on it, a drop of it, and the end of its storage.
///
/// These are the effects that [`walk_cfg`] applies, so that a value ends
/// at the same locations in both analyses.
pub fn collect_value_ends(
    basic_blocks: &IndexMap<BasicBlockId, MirBasicBlock>,
) -> HashSet<(Location, LocalId)> {
    basic_blocks
        .iter()
        .flat_map(|(block, bb_data)| {
            block_effects(bb_data).into_iter().enumerate().flat_map(
                move |(statement_index, effects)| {
                    let location = Location::from((*block, statement_index));
                    effects
                        .into_iter()
                        .filter(|(_, effect)| effect.value_ends())
                        .map(move |(place, _)| (location, LocalId::from(place.local)))
                },
            )
        })
        .collect()
}

pub type BasicBlocks = IndexMap<BasicBlockId, MirBasicBlock>;

#[allow(clippy::type_complexity)]
fn collect_places_effects(
    basic_blocks: &BasicBlocks,
) -> (Places, IndexMap<BasicBlockId, Vec<Vec<(PlaceId, Effect)>>>) {
    let effects: IndexMap<_, _> = basic_blocks
        .iter()
        .map(|(block, bb_data)| (*block, block_effects(bb_data)))
        .collect();
    let places = Places::new(effects.values().flatten().flatten().map(|(place, _)| place));
    // each effect on the places that it applies to
    let effects = effects
        .iter()
        .map(|(block, effects)| {
            let effects = effects
                .iter()
                .map(|effects| {
                    effects
                        .iter()
                        .flat_map(|(place, effect)| {
                            places
                                .id(place)
                                .into_iter()
                                .flat_map(|id| places.covered(id))
                                .map(|id| (id, *effect))
                        })
                        .collect()
                })
                .collect();
            (*block, effects)
        })
        .collect();
    (places, effects)
}

/// The state of each local as a whole after each [`Location`].
pub type CfgAnalysisOutput = IndexMap<Location, IndexMap<LocalId, StateBitSet>>;

/// Forward dataflow over the CFG of MIR [`Body`], returning the state of
/// each local after each [`Location`] at the least fixed point.
///
/// Starts at the entry block with every place marked `Uninitialized`. A
/// block is walked again from its entry states only when a predecessor adds
/// a state to them. Since the states only grow and each of them has at most
/// four variants, this terminates without any cutoff. The locations of
/// unreachable blocks keep empty states.
///
/// We may use [`rustc_mir_dataflow::impls::MaybeInitializedPlaces`] or such impls,
/// but some of them do not work as we expected. So we impl this analyzer.
pub fn walk_cfg(basic_blocks: &BasicBlocks) -> CfgAnalysisOutput {
    let (places, effects) = collect_places_effects(basic_blocks);

    let empty = places.states(StateBitSet::new());
    let mut states: IndexMap<Location, std::rc::Rc<States>> = IndexMap::new();
    for (block, effects) in &effects {
        for statement_index in 0..effects.len() {
            states.insert(
                Location::from((*block, statement_index)),
                std::rc::Rc::new(empty.clone()),
            );
        }
    }
    let mut entries: HashMap<BasicBlockId, States> = effects
        .keys()
        .map(|block| (*block, empty.clone()))
        .collect();

    let mut queued = VecDeque::new();
    if let Some(entry) = effects.keys().next() {
        let mut uninitialized = StateBitSet::new();
        uninitialized.insert(LocalStateVariant::Uninitialized);
        entries.insert(*entry, places.states(uninitialized));
        queued.push_back(*entry);
    }
    while let Some(block) = queued.pop_front() {
        let (Some(bb_data), Some(effects), Some(entry)) = (
            basic_blocks.get(&block),
            effects.get(&block),
            entries.get(&block),
        ) else {
            continue;
        };
        let mut current = entry.clone();
        // Statements with no effects leave the state untouched, so those all
        // share one allocation instead of a fresh copy each.
        let mut shared = std::rc::Rc::new(current.clone());
        for (statement_index, effects) in effects.iter().enumerate() {
            if effects.is_empty() {
                states.insert(Location::from((block, statement_index)), shared.clone());
                continue;
            }
            current.apply(effects);
            shared = std::rc::Rc::new(current.clone());
            states.insert(Location::from((block, statement_index)), shared.clone());
        }
        for successor in bb_data.terminator.successors() {
            if let Some(entry) = entries.get_mut(&successor)
                && entry.join(&current)
                && queued.iter().all(|v| *v != successor)
            {
                queued.push_back(successor);
            }
        }
    }
    states
        .iter()
        .map(|(location, states)| (*location, places.local_states(states)))
        .collect()
}

/// Source ranges where each local is definitely initialized.
///
/// A location qualifies when the per-local state set is exactly
/// `{Initialized}`, i.e. every path reaching the location leaves the local
/// in the initialized state. Surfaced as the green "definitely live"
/// decoration.
pub fn get_definitely_lives(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.len() == 1 && state.contains(LocalStateVariant::Initialized)
    })
}

/// Source ranges where each local is initialized on at least one path,
/// possibly together with `Moved`, `Dropped`, or `Uninitialized` on others.
///
/// This is a strict superset of [`get_definitely_lives`]; the difference
/// (a "maybe live" range) marks code where ownership is conditional on
/// control flow, which is the interesting case for auditing resource
/// cleanup such as `Drop` impls, file handles, or locks.
pub fn get_maybe_initialized(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
) -> HashMap<LocalId, Vec<Range>> {
    check_cfg_analysis_result(cfg_analysis_output, location_ranges, |state| {
        state.contains(LocalStateVariant::Initialized)
    })
}

fn check_cfg_analysis_result(
    cfg_analysis_output: &CfgAnalysisOutput,
    location_ranges: &LocationRanges,
    eval: impl Fn(&StateBitSet) -> bool,
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
