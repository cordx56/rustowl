use super::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum LocalStateVariant {
    Uninitialized = 0b0001,
    Initialized = 0b0010,
    Moved = 0b0100,
    Dropped = 0b1000,
}

/// Holds states using bit representation for performance
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct StateBitSet(u8);
impl StateBitSet {
    pub fn new() -> Self {
        Self(0)
    }
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    #[inline]
    pub fn remove(&mut self, variant: LocalStateVariant) {
        self.0 &= !(variant as u8);
    }
    #[inline]
    pub fn insert(&mut self, variant: LocalStateVariant) {
        self.0 |= variant as u8;
    }
    #[inline]
    pub fn contains(&self, variant: LocalStateVariant) -> bool {
        0 < self.0 & (variant as u8)
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }
    #[inline]
    pub fn extend(&mut self, other: Self) {
        self.0 |= other.0
    }

    /// Transition by an effect on the place.
    ///
    /// Note: a drop changes only the paths where the place holds a value,
    /// since it does nothing on the other paths; the other variants (e.g.
    /// an earlier `Moved`) survive, so that joins keep reflecting all paths
    /// reaching the location.
    pub fn apply(&mut self, effect: Effect) {
        match effect {
            Effect::Assign => {
                self.clear();
                self.insert(LocalStateVariant::Initialized);
            }
            Effect::Move => {
                self.clear();
                self.insert(LocalStateVariant::Moved);
            }
            Effect::Drop => {
                if self.contains(LocalStateVariant::Initialized) {
                    self.remove(LocalStateVariant::Initialized);
                    self.insert(LocalStateVariant::Dropped);
                }
            }
            Effect::StorageDead => {
                self.clear();
                self.insert(LocalStateVariant::Uninitialized);
            }
        }
    }

    /// The whole place is then exactly `{Initialized}` if and only if every
    /// one of them is, and has `Initialized` if and only if every one of them
    /// has.
    ///
    /// FIXME: The latter over-approximates since the fragments may be
    /// initialized on different paths.
    pub fn whole(parts: impl Iterator<Item = Self>) -> Self {
        let mut whole = Self::new();
        let mut all_initialized = true;
        for part in parts {
            all_initialized &= part.contains(LocalStateVariant::Initialized);
            whole.extend(part);
        }
        if !all_initialized {
            whole.remove(LocalStateVariant::Initialized);
        }
        whole
    }
}
