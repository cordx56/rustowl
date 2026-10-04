use super::*;
use indexmap::{IndexMap, IndexSet};
use std::collections::HashMap;

/// The index of a place in [`Places`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlaceId(usize);

/// The places distinguished by the analysis.
///
/// A place stands for its storage minus the storage of its fragments in
/// this table. An effect on a place also applies to its fragments,
/// and the whole of a place holds a value only if its fragments do.
pub struct Places {
    places: IndexMap<MirPlace, Vec<PlaceId>>,
}
impl Places {
    pub fn new<'a>(targets: impl Iterator<Item = &'a MirPlace>) -> Self {
        let places: IndexSet<_> = targets
            .flat_map(|target| [MirPlace::root(target.local), target.clone()])
            .collect();
        // a fragment shares the local, so only the places of the same local
        // are compared
        let mut local_places: HashMap<_, Vec<_>> = HashMap::new();
        for (index, place) in places.iter().enumerate() {
            local_places
                .entry(place.local)
                .or_default()
                .push(PlaceId(index));
        }
        let fragments: Vec<Vec<_>> = places
            .iter()
            .map(|place| {
                local_places[&place.local]
                    .iter()
                    .copied()
                    .filter(|other| is_fragment(place, &places[other.0]))
                    .collect()
            })
            .collect();
        Self {
            places: places.into_iter().zip(fragments).collect(),
        }
    }
    pub fn id(&self, place: &MirPlace) -> Option<PlaceId> {
        self.places.get_index_of(place).map(PlaceId)
    }
    /// The place and its fragments.
    pub fn covered(&self, id: PlaceId) -> impl Iterator<Item = PlaceId> {
        std::iter::once(id).chain(self.places[id.0].iter().copied())
    }
    /// [`States`] are made only by this, so that they always match the places.
    pub fn states(&self, state: StateBitSet) -> States {
        States(vec![state; self.places.len()])
    }
    /// The state of each local as a whole.
    pub fn local_states(&self, states: &States) -> IndexMap<LocalId, StateBitSet> {
        self.places
            .keys()
            .enumerate()
            .filter(|(_, place)| place.projection.is_empty())
            .map(|(index, place)| {
                let parts = self.covered(PlaceId(index)).map(|id| states[id]);
                (LocalId::from(place.local), StateBitSet::whole(parts))
            })
            .collect()
    }
}

/// The states of all the places, made by [`Places::states`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct States(Vec<StateBitSet>);
impl std::ops::Index<PlaceId> for States {
    type Output = StateBitSet;
    fn index(&self, id: PlaceId) -> &StateBitSet {
        &self.0[id.0]
    }
}
impl std::ops::IndexMut<PlaceId> for States {
    fn index_mut(&mut self, id: PlaceId) -> &mut StateBitSet {
        &mut self.0[id.0]
    }
}
impl States {
    /// Join operation for the lattice: per-place set union with `other`.
    /// Returns whether any state has grown.
    pub fn join(&mut self, other: &Self) -> bool {
        let mut grown = false;
        for (state, other) in self.0.iter_mut().zip(&other.0) {
            let before = *state;
            state.extend(*other);
            grown |= *state != before;
        }
        grown
    }
    pub fn apply(&mut self, effects: &[(PlaceId, Effect)]) {
        for (id, effect) in effects {
            self[*id].apply(*effect);
        }
    }
}
