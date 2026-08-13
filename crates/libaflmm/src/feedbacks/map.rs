//! Map feedback, maximizing or minimizing maps, for example the afl-style map observer.

#[cfg(feature = "simd")]
use super::simd::SimdMapFeedback;
use crate::{
    common::DependencyResolver,
    common::Registrator,
    corpus::TestcaseId,
    executors::ExitKind,
    feedbacks::{Feedback, HasObserverHandle},
    observers::MapObserver,
    states::{STAT_COVERAGE, State},
};
use alloc::{borrow::Cow, string::ToString, vec::Vec};
use core::{
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};
use hashbrown::HashMap;
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
use libaflmm_bolts::simd::vector::u8x16;
#[cfg(not(feature = "simd"))]
use libaflmm_bolts::simd::{MinReducer, OrReducer};
#[cfg(feature = "simd")]
use libaflmm_bolts::simd::{SimdMaxReducer, SimdMinReducer, SimdOrReducer, vector::u8x32};
use libaflmm_bolts::{
    AsIter, HasRefCnt, Named,
    simd::{MaxReducer, NopReducer, Reducer},
    tuples::{Handle, Handled, MatchName, MatchNameRef},
};
use libaflmm_core::Result;
use num_traits::PrimInt;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub type StdMapFeedback<C, O> = MaxMapFeedback<C, O>;

#[cfg(feature = "simd")]
/// A [`SimdMapFeedback`] that implements the AFL algorithm using an [`SimdOrReducer`] combining the bits for the history map and the bit from (`HitcountsMapObserver`)[`crate::observers::HitcountsMapObserver`].
pub type AflMapFeedback<C, O> = SimdMapFeedback<C, O, SimdOrReducer, u8x32>;
#[cfg(not(feature = "simd"))]
/// A [`MapFeedback`] that implements the AFL algorithm using an [`OrReducer`] combining the bits for the history map and the bit from (`HitcountsMapObserver`)[`crate::observers::HitcountsMapObserver`].
pub type AflMapFeedback<C, O> = MapFeedback<C, DifferentIsNovel, O, OrReducer>;

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
/// A [`SimdMapFeedback`] that strives to maximize the map contents.
pub type MaxMapFeedback<C, O> = SimdMapFeedback<C, O, SimdMaxReducer, u8x16>;
#[cfg(all(feature = "simd", not(target_arch = "x86_64")))]
/// A [`SimdMapFeedback`] that strives to maximize the map contents.
pub type MaxMapFeedback<C, O> = SimdMapFeedback<C, O, SimdMaxReducer, u8x32>;
#[cfg(not(feature = "simd"))]
/// A [`MapFeedback`] that strives to maximize the map contents.
pub type MaxMapFeedback<C, O> = MapFeedback<C, DifferentIsNovel, O, MaxReducer>;

#[cfg(feature = "simd")]
/// A [`SimdMapFeedback`] that strives to minimize the map contents.
pub type MinMapFeedback<C, O> = SimdMapFeedback<C, O, SimdMinReducer, u8x32>;
#[cfg(not(feature = "simd"))]
/// A [`MapFeedback`] that strives to minimize the map contents.
pub type MinMapFeedback<C, O> = MapFeedback<C, DifferentIsNovel, O, MinReducer>;

/// A [`MapFeedback`] that always returns `true` for `is_interesting`. Useful for tracing all executions.
pub type AlwaysInterestingMapFeedback<C, O> = MapFeedback<C, AllIsNovel, O, NopReducer>;

/// A [`MapFeedback`] that strives to maximize the map contents,
/// but only, if a value is larger than `pow2` of the previous.
pub type MaxMapPow2Feedback<C, O> = MapFeedback<C, NextPow2IsNovel, O, MaxReducer>;
/// A [`MapFeedback`] that strives to maximize the map contents,
/// but only, if a value is either `T::one()` or `T::max_value()`.
pub type MaxMapOneOrFilledFeedback<C, O> = MapFeedback<C, OneOrFilledIsNovel, O, MaxReducer>;

/// A `IsNovel` function is used to discriminate if a reduced value is considered novel.
pub trait IsNovel<T> {
    /// If a new value in the [`MapFeedback`] was found,
    /// this filter can decide if the result is considered novel or not.
    fn is_novel(old: T, new: T) -> bool;
}

/// [`AllIsNovel`] consider everything a novelty. Here mostly just for debugging.
#[derive(Debug, Clone)]
pub struct AllIsNovel {}

impl<T> IsNovel<T> for AllIsNovel
where
    T: Default + Copy + 'static,
{
    #[inline]
    fn is_novel(_old: T, _new: T) -> bool {
        true
    }
}

/// Calculate the next power of two
/// See <https://stackoverflow.com/a/66253960/1345238>
/// Will saturate at the max value.
/// In case of negative values, returns 1.
#[inline]
fn saturating_next_power_of_two<T: PrimInt>(n: T) -> T {
    if n <= T::one() {
        T::one()
    } else {
        (T::max_value() >> (n - T::one()).leading_zeros().try_into().unwrap())
            .saturating_add(T::one())
    }
}

/// Consider as novelty if the reduced value is different from the old value.
#[derive(Debug, Clone)]
pub struct DifferentIsNovel {}

impl<T> IsNovel<T> for DifferentIsNovel
where
    T: PartialEq + Default + Copy + 'static,
{
    #[inline]
    fn is_novel(old: T, new: T) -> bool {
        old != new
    }
}

/// Only consider as novel the values which are at least the next pow2 class of the old value
#[derive(Debug, Clone)]
pub struct NextPow2IsNovel {}

impl<T> IsNovel<T> for NextPow2IsNovel
where
    T: PrimInt + Default + Copy + 'static,
{
    #[inline]
    fn is_novel(old: T, new: T) -> bool {
        // We use a trait so we build our numbers from scratch here.
        // This way it works with Nums of any size.
        if new <= old {
            false
        } else {
            let pow2 = saturating_next_power_of_two(old.saturating_add(T::one()));
            new >= pow2
        }
    }
}

/// Only consider `T::one()` or `T::max_value()`, if they are bigger than the old value, as novel
#[derive(Debug, Clone)]
pub struct OneOrFilledIsNovel {}

impl<T> IsNovel<T> for OneOrFilledIsNovel
where
    T: PrimInt + Default + Copy + 'static,
{
    #[inline]
    fn is_novel(old: T, new: T) -> bool {
        (new == T::one() || new == T::max_value()) && new > old
    }
}

/// A testcase metadata holding a list of indexes of a map
#[derive(Debug, Serialize, Deserialize)]
pub struct MapIndexes {
    /// The actual list.
    pub list: Vec<usize>,
    /// A refcount used to know when we can remove this metadata
    pub tcref: isize,
}

/// A metadata mapping [`Testcase`](crate::corpus::Testcase)s to their respective [`MapIndexes`].
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MapIndexesMetadata {
    /// The actual map.
    pub data: HashMap<TestcaseId, MapIndexes>,
}

libaflmm_bolts::impl_serdeany!(MapIndexesMetadata);

impl Deref for MapIndexes {
    type Target = [usize];
    /// Convert to a slice
    fn deref(&self) -> &[usize] {
        &self.list
    }
}

impl DerefMut for MapIndexes {
    /// Convert to a slice
    fn deref_mut(&mut self) -> &mut [usize] {
        &mut self.list
    }
}

impl HasRefCnt for MapIndexes {
    fn refcnt(&self) -> isize {
        self.tcref
    }

    fn refcnt_mut(&mut self) -> &mut isize {
        &mut self.tcref
    }
}

impl MapIndexes {
    /// Creates a new [`struct@MapIndexesMetadata`].
    #[must_use]
    pub fn new(list: Vec<usize>) -> Self {
        Self { list, tcref: 0 }
    }
}

/// A testcase metadata holding a list of indexes of a map
#[derive(Debug, Serialize, Deserialize)]
pub struct MapNoveltiesMetadata {
    /// A `list` of novelties.
    pub list: Vec<usize>,
}

libaflmm_bolts::impl_serdeany!(MapNoveltiesMetadata);

impl Deref for MapNoveltiesMetadata {
    type Target = [usize];
    /// Convert to a slice
    fn deref(&self) -> &[usize] {
        &self.list
    }
}

impl DerefMut for MapNoveltiesMetadata {
    /// Convert to a slice
    fn deref_mut(&mut self) -> &mut [usize] {
        &mut self.list
    }
}

impl MapNoveltiesMetadata {
    /// Creates a new [`struct@MapNoveltiesMetadata`]
    #[must_use]
    pub fn new(list: Vec<usize>) -> Self {
        Self { list }
    }
}

/// The state of [`MapFeedback`]
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct MapFeedbackMetadata<T> {
    /// Contains information about untouched entries
    pub history_map: Vec<T>,
    /// Tells us how many non-initial entries there are in `history_map`
    pub num_covered_map_indexes: usize,
}

libaflmm_bolts::impl_serdeany!(
    MapFeedbackMetadata<T: 'static + Debug + Serialize + DeserializeOwned>,
    <u8>,<u16>,<u32>,<u64>,<i8>,<i16>,<i32>,<i64>,<f32>,<f64>,<bool>,<char>,<usize>
);

impl<T> MapFeedbackMetadata<T>
where
    T: Default + Copy + 'static + Serialize + DeserializeOwned + PartialEq,
{
    /// Create new `MapFeedbackMetadata`
    #[must_use]
    pub fn new(map_size: usize) -> Self {
        Self {
            history_map: vec![T::default(); map_size],
            num_covered_map_indexes: 0,
        }
    }

    /// Create new `MapFeedbackMetadata` using a name and a map.
    /// The map can be shared.
    /// `initial_elem_value` is used to calculate `Self.num_covered_map_indexes`
    #[must_use]
    pub fn with_history_map(history_map: Vec<T>, initial_elem_value: T) -> Self {
        let num_covered_map_indexes = history_map
            .iter()
            .fold(0, |acc, x| acc + usize::from(*x != initial_elem_value));
        Self {
            history_map,
            num_covered_map_indexes,
        }
    }

    /// Reset the map
    pub fn reset(&mut self) -> Result<()> {
        let cnt = self.history_map.len();
        for i in 0..cnt {
            self.history_map[i] = T::default();
        }
        self.num_covered_map_indexes = 0;
        Ok(())
    }

    /// Reset the map with any value
    pub fn reset_with_value(&mut self, value: T) -> Result<()> {
        let cnt = self.history_map.len();
        for i in 0..cnt {
            self.history_map[i] = value;
        }
        // assume that resetting the map should indicate no coverage,
        // regardless of value
        self.num_covered_map_indexes = 0;
        Ok(())
    }
}

/// The most common AFL-like feedback type
#[derive(Debug, Clone)]
pub struct MapFeedback<C, N, O, R> {
    /// Name identifier of this instance
    name: Cow<'static, str>,
    /// Name identifier of the observer
    map_ref: Handle<C>,
    /// Phantom Data of Reducer
    #[expect(clippy::type_complexity)]
    phantom: PhantomData<fn() -> (N, O, R)>,
}

impl<C, N, O, R> DependencyResolver for MapFeedback<C, N, O, R>
where
    O: MapObserver,
    O::Entry: 'static + Default + Debug + DeserializeOwned + Serialize,
{
    fn register_md(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_md_default::<MapFeedbackMetadata<O::Entry>>(self.name());
        Ok(())
    }
}

impl<C, I, N, O, OT, R, S> Feedback<I, OT, S> for MapFeedback<C, N, O, R>
where
    C: AsRef<O>,
    N: IsNovel<O::Entry>,
    O: MapObserver + for<'it> AsIter<'it, Item = O::Entry>,
    O::Entry: 'static + Default + Debug + DeserializeOwned + Serialize,
    OT: MatchName,
    R: Reducer<O::Entry>,
    S: State<Input = I>,
{
    fn is_interesting(
        &mut self,
        state: &mut S,
        _input: &I,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool> {
        let res = self.is_interesting_default(state, observers);

        Ok(res)
    }

    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        let observer = observers.get(&self.map_ref).expect("MapObserver not found. This is likely because you entered the crash handler with the wrong executor/observer").as_ref();
        let initial = observer.initial();
        let map_state = state
            .metadata_map_mut()
            .get_mut::<MapFeedbackMetadata<O::Entry>>(&self.name)
            .unwrap();
        let len = observer.len();
        if map_state.history_map.len() < len {
            map_state.history_map.resize(len, observer.initial());
        }

        let map_len = map_state.history_map.len();

        let history_map = &mut map_state.history_map;
        for (i, value) in observer
            .as_iter()
            .map(|x| *x)
            .enumerate()
            .filter(|(_, value)| *value != initial)
        {
            let val = R::reduce(history_map[i], value);
            if history_map[i] == initial && val != initial {
                map_state.num_covered_map_indexes += 1;
            }
            history_map[i] = val;
        }

        debug_assert!(
            history_map
                .iter()
                .fold(0, |acc, x| acc + usize::from(*x != initial))
                == map_state.num_covered_map_indexes,
            "history_map had {} filled, but map_state.num_covered_map_indexes was {}",
            history_map
                .iter()
                .fold(0, |acc, x| acc + usize::from(*x != initial)),
            map_state.num_covered_map_indexes,
        );

        let covered = map_state.num_covered_map_indexes;
        let stat_json = serde_json::json!([covered, map_len]).to_string();

        let stats_name = format!("{}-{}", STAT_COVERAGE, self.name().to_string());

        state.stats_mut().user_map.insert(stats_name, stat_json);

        Ok(())
    }
}

impl<C, N, O, R> Named for MapFeedback<C, N, O, R> {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<C, N, O, R> MapFeedback<C, N, O, R>
where
    C: AsRef<O> + Named,
{
    /// Create new `MapFeedback`
    #[must_use]
    pub fn new(map_observer: &C) -> Self {
        Self {
            name: map_observer.name().clone(),
            map_ref: map_observer.handle(),
            phantom: PhantomData,
        }
    }

    /// Creating a new `MapFeedback` with a specific name. This is usefully whenever the same
    /// feedback is needed twice, but with a different history. Using `new()` always results in the
    /// same name and therefore also the same history.
    #[must_use]
    pub fn with_name(name: &'static str, map_observer: &C) -> Self {
        let name = Cow::from(name);
        Self {
            map_ref: map_observer.handle(),
            name,
            phantom: PhantomData,
        }
    }
}

impl<C, N, O, R> HasObserverHandle for MapFeedback<C, N, O, R> {
    type Observer = C;

    #[inline]
    fn observer_handle(&self) -> &Handle<C> {
        &self.map_ref
    }
}

impl<C, N, O, R> MapFeedback<C, N, O, R>
where
    R: Reducer<O::Entry>,
    O: MapObserver + for<'it> AsIter<'it, Item = O::Entry>,
    O::Entry: 'static + Debug + Serialize + DeserializeOwned,
    N: IsNovel<O::Entry>,
    C: AsRef<O>,
{
    fn is_interesting_default<OT, S>(&mut self, state: &mut S, observers: &OT) -> bool
    where
        S: State,
        OT: MatchName,
    {
        let mut interesting = false;
        // TODO Replace with match_name_type when stable
        let observer = observers.get(&self.map_ref).unwrap().as_ref();

        let map_state = state
            .metadata_map_mut()
            .get_mut::<MapFeedbackMetadata<O::Entry>>(&self.name)
            .unwrap();
        let len = observer.len();
        if map_state.history_map.len() < len {
            map_state.history_map.resize(len, observer.initial());
        }

        let history_map = map_state.history_map.as_slice();

        let initial = observer.initial();

        for (i, item) in observer
            .as_iter()
            .map(|x| *x)
            .enumerate()
            .filter(|(_, item)| *item != initial)
        {
            let existing = unsafe { *history_map.get_unchecked(i) };
            let reduced = R::reduce(existing, item);
            if N::is_novel(existing, reduced) {
                interesting = true;
                break;
            }
        }

        interesting
    }
}

#[cfg(test)]
mod tests {
    use crate::feedbacks::{AllIsNovel, IsNovel, NextPow2IsNovel};

    #[test]
    fn test_map_is_novel() {
        // This should always hold
        assert!(AllIsNovel::is_novel(0_u8, 0));

        assert!(!NextPow2IsNovel::is_novel(0_u8, 0));
        assert!(NextPow2IsNovel::is_novel(0_u8, 1));
        assert!(!NextPow2IsNovel::is_novel(1_u8, 1));
        assert!(NextPow2IsNovel::is_novel(1_u8, 2));
        assert!(!NextPow2IsNovel::is_novel(2_u8, 2));
        assert!(!NextPow2IsNovel::is_novel(2_u8, 3));
        assert!(NextPow2IsNovel::is_novel(2_u8, 4));
        assert!(!NextPow2IsNovel::is_novel(128_u8, 128));
        assert!(!NextPow2IsNovel::is_novel(129_u8, 128));
        assert!(NextPow2IsNovel::is_novel(128_u8, 255));
        assert!(!NextPow2IsNovel::is_novel(255_u8, 128));
        assert!(NextPow2IsNovel::is_novel(254_u8, 255));
        assert!(!NextPow2IsNovel::is_novel(255_u8, 255));
    }
}
