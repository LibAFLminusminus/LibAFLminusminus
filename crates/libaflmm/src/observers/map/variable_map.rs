//! A variable length map observer

use crate::{
    Result,
    common::DependencyResolver,
    observers::{MapObserver, Observer},
};
use alloc::{borrow::Cow, fmt::Debug};
use core::{
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};
use libaflmm_bolts::OwnedMutSlice;
use libaflmm_core::{AsSlice, AsSliceMut, HasLen, Named, Truncate};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The Map Observer retrieves the state of a map,
/// that will get updated by the target.
/// A well-known example is the AFL-Style coverage map.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[expect(clippy::unsafe_derive_deserialize)]
pub struct VariableMapObserver<'a, T> {
    map: OwnedMutSlice<'a, T>,
    initial: T,
    name: Cow<'static, str>,
}

impl<T> DependencyResolver for VariableMapObserver<'_, T> {}

impl<S, T> Observer<S> for VariableMapObserver<'_, T>
where
    Self: MapObserver,
{
    #[inline]
    fn pre_exec(&mut self, _state: &mut S) -> Result<()> {
        self.reset_map()
    }
}

impl<T> Named for VariableMapObserver<'_, T> {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<T> HasLen for VariableMapObserver<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.map.as_slice().len()
    }
}

impl<T> Hash for VariableMapObserver<'_, T>
where
    T: Hash,
{
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.as_slice().hash(hasher);
    }
}

impl<T> AsRef<Self> for VariableMapObserver<'_, T> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T> AsMut<Self> for VariableMapObserver<'_, T> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T> MapObserver for VariableMapObserver<'_, T>
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
    fn reset_map(&mut self) -> Result<()> {
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

impl<T> Truncate for VariableMapObserver<'_, T> {
    fn truncate(&mut self, new_len: usize) {
        self.map.truncate(new_len);
    }
}

impl<T> Deref for VariableMapObserver<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.map
    }
}

impl<T> DerefMut for VariableMapObserver<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.map
    }
}

impl<'a, T> VariableMapObserver<'a, T>
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
        VariableMapObserver {
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
