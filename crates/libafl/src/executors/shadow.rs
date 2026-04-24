//! A `ShadowExecutor` wraps an executor to have shadow observer that will not be considered by the feedbacks and the manager

use core::{
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    time::Duration,
};

use libafl_bolts::tuples::RefIndexable;

use crate::{
    DependencyResolver, Error,
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
};

/// A [`ShadowExecutor`] wraps an executor and a set of shadow observers
pub struct ShadowExecutor<E, I, S, SOT> {
    /// The wrapped executor
    executor: E,
    /// The shadow observers
    shadow_observers: SOT,
    phantom: PhantomData<(I, S)>,
}

impl<E, I, S, SOT> Debug for ShadowExecutor<E, I, S, SOT>
where
    E: Debug,
    SOT: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShadowExecutor")
            .field("executor", &self.executor)
            .field("shadow_observers", &self.shadow_observers)
            .finish()
    }
}

impl<E, I, S, SOT> ShadowExecutor<E, I, S, SOT>
where
    E: Executor<I, S>,
    SOT: ObserversTuple<S>,
{
    /// Create a new `ShadowExecutor`, wrapping the given `executor`.
    pub fn new(executor: E, shadow_observers: SOT) -> Self {
        Self {
            executor,
            shadow_observers,
            phantom: PhantomData,
        }
    }

    /// The shadow observers are not considered by the feedbacks and the manager, mutable
    #[inline]
    pub fn shadow_observers(&self) -> RefIndexable<&SOT, SOT> {
        RefIndexable::from(&self.shadow_observers)
    }

    /// The shadow observers are not considered by the feedbacks and the manager, mutable
    #[inline]
    pub fn shadow_observers_mut(&mut self) -> RefIndexable<&mut SOT, SOT> {
        RefIndexable::from(&mut self.shadow_observers)
    }

    /// Inner executor
    #[inline]
    pub fn executor(&self) -> &E {
        &self.executor
    }

    /// Inner executor
    #[inline]
    pub fn executor_mut(&mut self) -> &mut E {
        &mut self.executor
    }
}

impl<E, I, S, SOT> DependencyResolver for ShadowExecutor<E, I, S, SOT> {}

impl<E, I, S, SOT> Executor<I, S> for ShadowExecutor<E, I, S, SOT>
where
    E: Executor<I, S>,
    SOT: ObserversTuple<S>,
{
    type Observers = E::Observers;

    fn init<CT: crate::Worker>(
        &mut self,
        state: &mut S,
        rt_handle: &mut crate::runtimes::RuntimeHandle<CT, S>,
    ) -> Result<(), Error> {
        self.executor.init(state, rt_handle)
    }

    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind, Error> {
        self.executor.execute_impl(state, input)
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        self.executor.observers()
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        self.executor.observers_mut()
    }
}
