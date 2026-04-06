//! An observer that takes multiple pointers or slices to observe

use alloc::{borrow::Cow, vec::Vec};
use core::{
    fmt::Debug,
    hash::{Hash, Hasher},
    iter::Flatten,
    slice::{Iter, IterMut},
};

use libafl_bolts::{
    AsIter, AsIterMut, AsSlice, AsSliceMut, HasLen, Named, ownedref::OwnedMutSlice,
};
use meminterval::IntervalTree;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    Error,
    observers::{Observer, map::MapObserver},
};

/// The Multi Map Observer merge different maps into one observer
#[derive(Serialize, Deserialize, Debug)]
pub struct MultiMapObserver<'a, T> {
    maps: Vec<OwnedMutSlice<'a, T>>,
    intervals: IntervalTree<usize, usize>,
    len: usize,
    initial: T,
    name: Cow<'static, str>,
    iter_idx: usize,
}

impl<S, T> Observer<S> for MultiMapObserver<'_, T>
where
    Self: MapObserver,
{
    #[inline]
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.reset_map()
    }
}

impl<T> Named for MultiMapObserver<'_, T> {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<T> HasLen for MultiMapObserver<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

impl<T> Hash for MultiMapObserver<'_, T>
where
    T: Hash,
{
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        for map in &self.maps {
            let slice = map.as_slice();

            slice.hash(hasher);
        }
    }
}

impl<T> AsRef<Self> for MultiMapObserver<'_, T> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T> AsMut<Self> for MultiMapObserver<'_, T> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T> MapObserver for MultiMapObserver<'_, T>
where
    T: PartialEq + Copy + Hash + Serialize + DeserializeOwned + Debug,
{
    type Entry = T;

    #[inline]
    fn get(&self, idx: usize) -> T {
        let elem = self.intervals.query(idx..=idx).next().unwrap();
        let i = *elem.value;
        let j = idx - elem.interval.start;
        self.maps[i].as_slice()[j]
    }

    #[inline]
    fn set(&mut self, idx: usize, val: Self::Entry) {
        let elem = self.intervals.query(idx..=idx).next().unwrap();
        let i = *elem.value;
        let j = idx - elem.interval.start;
        self.maps[i].as_slice_mut()[j] = val;
    }

    #[inline]
    fn initial(&self) -> T {
        self.initial
    }

    fn count_bytes(&self) -> u64 {
        let initial = self.initial();
        let mut res = 0;
        for map in &self.maps {
            for x in map.as_slice() {
                if *x != initial {
                    res += 1;
                }
            }
        }
        res
    }

    fn reset_map(&mut self) -> Result<(), Error> {
        let initial = self.initial();
        for map in &mut self.maps {
            for x in map.as_slice_mut() {
                *x = initial;
            }
        }
        Ok(())
    }

    fn usable_count(&self) -> usize {
        self.len()
    }

    fn to_vec(&self) -> Vec<Self::Entry> {
        let cnt = self.usable_count();
        let mut res = Vec::with_capacity(cnt);
        for i in 0..cnt {
            res.push(self.get(i));
        }
        res
    }

    /// Get the number of set entries with the specified indexes
    fn how_many_set(&self, indexes: &[usize]) -> usize {
        let initial = self.initial();
        let cnt = self.usable_count();
        let mut res = 0;
        for i in indexes {
            if *i < cnt && self.get(*i) != initial {
                res += 1;
            }
        }
        res
    }
}

impl<'a, T> MultiMapObserver<'a, T>
where
    T: Default,
{
    /// Creates a new [`MultiMapObserver`]
    #[must_use]
    fn new(name: &'static str, maps: Vec<OwnedMutSlice<'a, T>>) -> Self {
        let mut idx = 0;
        let mut intervals = IntervalTree::new();
        for (v, x) in maps.iter().enumerate() {
            let l = x.as_slice().len();
            intervals.insert(idx..(idx + l), v);
            idx += l;
        }
        Self {
            maps,
            intervals,
            len: idx,
            name: Cow::from(name),
            initial: T::default(),
            iter_idx: 0,
        }
    }

    /// Creates a new [`MultiMapObserver`] with an owned map
    #[must_use]
    pub fn owned(name: &'static str, maps: Vec<Vec<T>>) -> Self {
        let mut idx = 0;
        let mut v = 0;
        let mut intervals = IntervalTree::new();
        let maps: Vec<_> = maps
            .into_iter()
            .map(|x| {
                let l = x.len();
                intervals.insert(idx..(idx + l), v);
                idx += l;
                v += 1;
                OwnedMutSlice::from(x)
            })
            .collect();
        Self {
            maps,
            intervals,
            len: idx,
            name: Cow::from(name),
            initial: T::default(),
            iter_idx: 0,
        }
    }
}

impl<'a, 'it, T> AsIter<'it> for MultiMapObserver<'a, T>
where
    T: 'a,
    'a: 'it,
{
    type Item = T;
    type Ref = &'it T;
    type IntoIter = Flatten<Iter<'it, OwnedMutSlice<'a, T>>>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.maps.iter().flatten()
    }
}

impl<'a, 'it, T> AsIterMut<'it> for MultiMapObserver<'a, T>
where
    T: 'a,
    'a: 'it,
{
    type RefMut = &'it mut T;
    type IntoIterMut = Flatten<IterMut<'it, OwnedMutSlice<'a, T>>>;

    fn as_iter_mut(&'it mut self) -> Self::IntoIterMut {
        self.maps.iter_mut().flatten()
    }
}
