//! All the map observer variants

use alloc::{borrow::Cow, vec::Vec};
use core::{
    fmt::Debug,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

use libaflmm_bolts::{AsSlice, AsSliceMut, HasLen, Named, Truncate, ownedref::OwnedMutSlice};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{DependencyResolver, Error, observers::Observer};

pub mod const_map;
pub use const_map::*;

pub mod variable_map;
pub use variable_map::*;

pub mod hitcount_map;
pub use hitcount_map::*;

pub mod multi_map;
pub use multi_map::*;

/// A [`MapObserver`] observes the static map, as oftentimes used for AFL-like coverage information
///
/// When referring to this type in a constraint (e.g. `O: MapObserver`), ensure that you only refer
/// to instances of a second type, e.g. `C: AsRef<O>` or `A: AsMut<O>`. Map observer instances are
/// passed around in a way that may be potentially wrapped by e.g. [`ExplicitTracking`] as a way to
/// encode metadata into the type. This is an unfortunate additional requirement that we can't get
/// around without specialization.
///
/// See [`crate::require_index_tracking`] for an example of how to do so.
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
    fn reset_map(&mut self) -> Result<(), Error>;

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

/// The Map Observer retrieves the state of a map,
/// that will get updated by the target.
/// A well-known example is the AFL-Style coverage map.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[expect(clippy::unsafe_derive_deserialize)]
pub struct StdMapObserver<'a, T> {
    map: OwnedMutSlice<'a, T>,
    initial: T,
    name: Cow<'static, str>,
}

impl<T> DependencyResolver for StdMapObserver<'_, T> {}

impl<S, T> Observer<S> for StdMapObserver<'_, T>
where
    Self: MapObserver,
{
    #[inline]
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.reset_map()
    }
}

impl<T> Named for StdMapObserver<'_, T> {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<T> HasLen for StdMapObserver<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.map.as_slice().len()
    }
}

impl<T> Hash for StdMapObserver<'_, T>
where
    T: Hash,
{
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.as_slice().hash(hasher);
    }
}

impl<T> AsRef<Self> for StdMapObserver<'_, T> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T> AsMut<Self> for StdMapObserver<'_, T> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T> MapObserver for StdMapObserver<'_, T>
where
    T: PartialEq + Copy + Hash + Serialize + DeserializeOwned + Debug,
{
    type Entry = T;

    #[inline]
    fn get(&self, pos: usize) -> T {
        self.as_slice()[pos]
    }

    fn set(&mut self, pos: usize, val: T) {
        self.map.as_slice_mut()[pos] = val;
    }

    /// Count the set bytes in the map
    fn count_bytes(&self) -> u64 {
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.as_slice();
        let mut res = 0;
        for x in &map[0..cnt] {
            if *x != initial {
                res += 1;
            }
        }
        res
    }

    #[inline]
    fn usable_count(&self) -> usize {
        self.as_slice().len()
    }

    #[inline]
    fn initial(&self) -> T {
        self.initial
    }

    fn to_vec(&self) -> Vec<T> {
        self.as_slice().to_vec()
    }

    /// Reset the map
    #[inline]
    fn reset_map(&mut self) -> Result<(), Error> {
        // Normal memset, see https://rust.godbolt.org/z/Trs5hv
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.as_slice_mut();
        for x in &mut map[0..cnt] {
            *x = initial;
        }
        Ok(())
    }

    fn how_many_set(&self, indexes: &[usize]) -> usize {
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.as_slice();
        let mut res = 0;
        for i in indexes {
            if *i < cnt && map[*i] != initial {
                res += 1;
            }
        }
        res
    }
}

impl<T> Truncate for StdMapObserver<'_, T> {
    fn truncate(&mut self, new_len: usize) {
        self.map.truncate(new_len);
    }
}

impl<T> Deref for StdMapObserver<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.map
    }
}

impl<T> DerefMut for StdMapObserver<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.map
    }
}

impl<'a, T> StdMapObserver<'a, T>
where
    T: Default,
{
    /// Creates a new [`MapObserver`]
    ///
    /// # Safety
    /// Will get a pointer to the map and dereference it at any point in time.
    /// The map must not move in memory!
    #[must_use]
    pub unsafe fn new<S>(name: S, map: &'a mut [T]) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        unsafe {
            let len = map.len();
            let ptr = map.as_mut_ptr();
            Self::from_mut_ptr(name, ptr, len)
        }
    }

    /// Creates a new [`MapObserver`] from an [`OwnedMutSlice`]
    #[must_use]
    pub fn from_mut_slice<S>(name: S, map: OwnedMutSlice<'a, T>) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        StdMapObserver {
            name: name.into(),
            map,
            initial: T::default(),
        }
    }

    /// Creates a new [`MapObserver`] with an owned map
    #[must_use]
    pub fn owned<S>(name: S, map: Vec<T>) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self {
            map: OwnedMutSlice::from(map),
            name: name.into(),
            initial: T::default(),
        }
    }

    /// Creates a new [`MapObserver`] from an [`OwnedMutSlice`] map.
    ///
    /// # Safety
    /// Will dereference the owned slice with up to len elements.
    #[must_use]
    pub fn from_ownedref<S>(name: S, map: OwnedMutSlice<'a, T>) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self {
            map,
            name: name.into(),
            initial: T::default(),
        }
    }

    /// Creates a new [`MapObserver`] from a raw pointer
    ///
    /// # Safety
    /// Will dereference the `map_ptr` with up to len elements.
    pub unsafe fn from_mut_ptr<S>(name: S, map_ptr: *mut T, len: usize) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        unsafe { Self::from_mut_slice(name, OwnedMutSlice::from_raw_parts_mut(map_ptr, len)) }
    }

    /// Gets the initial value for this map, mutably
    pub fn initial_mut(&mut self) -> &mut T {
        &mut self.initial
    }

    /// Gets the backing for this map
    pub fn map(&self) -> &OwnedMutSlice<'a, T> {
        &self.map
    }

    /// Gets the backing for this map mutably
    pub fn map_mut(&mut self) -> &mut OwnedMutSlice<'a, T> {
        &mut self.map
    }
}
