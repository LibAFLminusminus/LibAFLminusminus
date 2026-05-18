//! SIMD accelerated map feedback with stable Rust.

use alloc::borrow::Cow;
use core::{
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use libaflmm_bolts::{
    AsIter, AsSlice, Named,
    simd::{Reducer, SimdReducer, VectorType, covmap_is_interesting_simd},
    tuples::{Handle, MatchName, MatchNameRef},
};
use libaflmm_core::Result;
use serde::{Serialize, de::DeserializeOwned};

use super::{DifferentIsNovel, Feedback, HasObserverHandle, MapFeedback};
use crate::{
    common::DependencyResolver,
    corpus::TestcaseId,
    executors::ExitKind,
    feedbacks::MapFeedbackMetadata,
    observers::MapObserver,
    states::{CoreState, State},
};

/// Stable Rust wrapper for SIMD accelerated map feedback. Unfortunately, we have to
/// keep this until specialization is stablized (not yet since 2016).
#[derive(Debug, Clone)]
pub struct SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
{
    map: MapFeedback<C, DifferentIsNovel, O, R::PrimitiveReducer>,
    _ph: PhantomData<V>,
}

impl<C, O, R, V> SimdMapFeedback<C, O, R, V>
where
    O: MapObserver<Entry = u8> + for<'a> AsSlice<'a, Entry = u8> + for<'a> AsIter<'a, Item = u8>,
    C: AsRef<O>,
    R: SimdReducer<V>,
    V: VectorType + Copy + Eq,
{
    fn is_interesting_u8_simd_optimized<S, OT>(&mut self, state: &mut S, observers: &OT) -> bool
    where
        S: CoreState,
        OT: MatchName,
    {
        // TODO Replace with match_name_type when stable
        let observer = observers.get(self.map.observer_handle()).expect("MapObserver not found. This is likely because you entered the crash handler with the wrong executor/observer").as_ref();

        let map_state = state
            .metadata_map_mut()
            .get_mut::<MapFeedbackMetadata<u8>>(self.map.name())
            .unwrap();
        let size = observer.usable_count();
        let len = observer.len();
        if map_state.history_map.len() < len {
            map_state.history_map.resize(len, u8::default());
        }

        let map = observer.as_slice();
        debug_assert!(map.len() >= size);

        let history_map = map_state.history_map.as_slice();

        unsafe { covmap_is_interesting_simd::<R, V>(history_map, &map) }
    }
}

impl<C, O, R, V> SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
{
    /// Wraps an existing map and enable SIMD acceleration. This will use standard SIMD
    /// implementation, which might vary based on target architecture according to our
    /// benchmark.
    #[must_use]
    pub fn wrap(map: MapFeedback<C, DifferentIsNovel, O, R::PrimitiveReducer>) -> Self {
        Self {
            map,
            _ph: PhantomData,
        }
    }
}

/// Implementation that mocks [`MapFeedback`], note the bound of O is intentionally stricter
/// than we we need to hint users when their entry is not `u8`. Without this bound, there
/// would be bound related errors in [`crate::fuzzers::StdFuzzer`], which is super confusing
/// and misleading.
impl<C, O, R, V> SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
    C: AsRef<O> + Named,
    O: MapObserver<Entry = u8> + for<'a> AsSlice<'a, Entry = u8> + for<'a> AsIter<'a, Item = u8>,
{
    /// Mock [`MapFeedback::new`]. If you are getting bound errors, your entry is probably not
    /// `u8` and you should use [`MapFeedback`] instead.
    #[must_use]
    pub fn new(map_observer: &C) -> Self {
        let map = MapFeedback::new(map_observer);
        Self {
            map,
            _ph: PhantomData,
        }
    }

    /// Mock [`MapFeedback::with_name`] If you are getting bound errors, your entry is probably not
    /// `u8` and you should use [`MapFeedback`] instead.
    #[must_use]
    pub fn with_name(name: &'static str, map_observer: &C) -> Self {
        let map = MapFeedback::with_name(name, map_observer);
        Self {
            map,
            _ph: PhantomData,
        }
    }
}

impl<C, O, R, V> Deref for SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
{
    type Target = MapFeedback<C, DifferentIsNovel, O, R::PrimitiveReducer>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<C, O, R, V> DerefMut for SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl<C, O, R, V> DependencyResolver for SimdMapFeedback<C, O, R, V>
where
    O: MapObserver,
    O::Entry: 'static + Default + Debug + DeserializeOwned + Serialize,
    R: SimdReducer<V>,
{
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        registrator.register_md_default::<MapFeedbackMetadata<u8>>("MapFeedback");

        self.map.register(registrator)
    }
}

impl<C, O, R, V> HasObserverHandle for SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
{
    type Observer = C;

    #[inline]
    fn observer_handle(&self) -> &Handle<C> {
        self.map.observer_handle()
    }
}

impl<C, O, R, V> Named for SimdMapFeedback<C, O, R, V>
where
    R: SimdReducer<V>,
{
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        self.map.name()
    }
}

// Delegate implementations to inner mapping except is_interesting
impl<C, O, I, OT, S, R, V> Feedback<I, OT, S> for SimdMapFeedback<C, O, R, V>
where
    C: AsRef<O>,
    O: MapObserver<Entry = u8> + for<'a> AsSlice<'a, Entry = u8> + for<'a> AsIter<'a, Item = u8>,
    OT: MatchName,
    R: SimdReducer<V>,
    S: State<Input = I>,
    V: VectorType + Copy + Eq,
    R::PrimitiveReducer: Reducer<u8>,
{
    fn is_interesting(
        &mut self,
        state: &mut S,
        _input: &I,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool> {
        let res = self.is_interesting_u8_simd_optimized(state, observers);
        Ok(res)
    }

    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.map.append_metadata(state, observers, testcase_id)
    }
}
