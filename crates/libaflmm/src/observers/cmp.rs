//! The [`CmpObserver`] provides access to the logged values of CMP instructions

/// A [`CmpObserver`] observes the traced comparisons during the current execution using a [`Self::Map`]
pub trait CmpObserver {
    /// The underlying map
    type Map;
    /// Get the number of usable cmps (all by default)
    fn usable_count(&self) -> usize;

    /// Get the underlying [`Self::Map`]
    fn cmp_map(&self) -> &Self::Map;

    /// Get the mut underlying [`Self::Map`]
    fn cmp_map_mut(&mut self) -> &mut Self::Map;
}
