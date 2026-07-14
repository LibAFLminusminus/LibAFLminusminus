//! All the map observer variants

use crate::Result;
use alloc::vec::Vec;
use core::fmt::Debug;
use libaflmm_bolts::{HasLen, Named};
use serde::{Serialize, de::DeserializeOwned};

pub mod const_map;
pub use const_map::ConstMapObserver;

pub mod variable_map;
pub use variable_map::VariableMapObserver;

pub mod size_ptr;
pub use size_ptr::SizePtrMapObserver;

pub mod hitcount_map;
pub use hitcount_map::{HitcountsIterableMapObserver, HitcountsMapObserver};

pub mod multi_map;
pub use multi_map::MultiMapObserver;

pub type StdMapObserver<'a, T> = VariableMapObserver<'a, T>;

/// A [`MapObserver`] observes the static map, as oftentimes used for AFL-like coverage information
///
/// TODO: enforce `iter() -> AssociatedTypeIter` when generic associated types stabilize
pub trait MapObserver:
    HasLen + Named + Serialize + DeserializeOwned + AsRef<Self> + AsMut<Self>
// where
//     for<'it> &'it Self: IntoIterator<Item = &'it Self::Entry>
{
    /// Type of each entry in this map
    type Entry: PartialEq + Copy + Debug;

    /// Get the value at `idx`
    fn get(&self, idx: usize) -> Self::Entry;

    /// Set the value at `idx`
    fn set(&mut self, idx: usize, val: Self::Entry);

    /// Get the number of usable entries in the map (all by default)
    fn usable_count(&self) -> usize;

    /// Count the set bytes in the map
    fn count_bytes(&self) -> u64;

    /// Get the initial value for `reset()`
    fn initial(&self) -> Self::Entry;

    /// Reset the map
    fn reset_map(&mut self) -> Result<()>;

    /// Get these observer's contents as [`Vec`]
    fn to_vec(&self) -> Vec<Self::Entry>;

    /// Get the number of set entries with the specified indexes
    fn how_many_set(&self, indexes: &[usize]) -> usize;
}

/// The "real" length of the underlying map could change at any point in time.
/// Thus, the size of the map should be fetched each time it is used.
pub trait VarLenMapObserver: MapObserver {
    /// A mutable slice reference to the map.
    /// The length of the map gives the maximum allocatable size.
    fn map_slice(&self) -> &[Self::Entry];

    /// A slice reference to the map.
    /// The length of the map gives the maximum allocatable size.
    fn map_slice_mut(&mut self) -> &mut [Self::Entry];

    /// A reference to the size of the map.
    fn size(&self) -> &usize;

    /// A mutable reference to the size of the map.
    fn size_mut(&mut self) -> &mut usize;
}

/// Implementors guarantee the size of the map is constant at any point in time and equals N.
pub trait ConstLenMapObserver<const N: usize>: MapObserver {
    /// The size of the map
    const LENGTH: usize = N;

    /// A mutable slice reference to the map
    fn map_slice(&self) -> &[Self::Entry; N];

    /// A mutable slice reference to the map
    fn map_slice_mut(&mut self) -> &mut [Self::Entry; N];
}
