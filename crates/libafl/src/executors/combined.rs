//! A `CombinedExecutor` wraps a primary executor and a secondary one
//! In comparison to the [`crate::executors::DiffExecutor`] it does not run the secondary executor in `run_target`.

use crate::{
    DependencyResolver, Result, Worker,
    executors::{Executor, ExitKind},
};
use core::{fmt::Debug, time::Duration};
use libafl_bolts::tuples::RefIndexable;

/// A [`CombinedExecutor`] wraps a primary executor, forwarding its methods, and a secondary one
#[derive(Debug)]
pub struct CombinedExecutor<A, B> {
    primary: A,
    secondary: B,
}

impl<A, B> CombinedExecutor<A, B> {
    /// Create a new `CombinedExecutor`, wrapping the given `executor`s.
    pub fn new(primary: A, secondary: B) -> Self {
        Self { primary, secondary }
    }

    /// Retrieve the primary `Executor` that is wrapped by this `CombinedExecutor`.
    pub fn primary(&mut self) -> &mut A {
        &mut self.primary
    }

    /// Retrieve the secondary `Executor` that is wrapped by this `CombinedExecutor`.
    pub fn secondary(&mut self) -> &mut B {
        &mut self.secondary
    }
}

impl<A, B> DependencyResolver for CombinedExecutor<A, B> {}

impl<A, B, I, S> Executor<I, S> for CombinedExecutor<A, B>
where
    A: Executor<I, S>,
    B: Executor<I, S>,
{
    type Observers = A::Observers;

    fn init<W: Worker>(
        &mut self,
        state: &mut S,
        rt_handle: &mut crate::runtimes::RuntimeHandle<S, W>,
    ) -> core::result::Result<(), libafl_core::Error> {
        self.primary.init(state, rt_handle)
    }

    unsafe fn execute_impl(
        &mut self,
        state: &mut S,
        input: &I,
    ) -> core::result::Result<ExitKind, libafl_core::Error> {
        self.primary.execute_impl(state, input)
    }

    #[inline]
    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        self.primary.observers()
    }

    #[inline]
    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        self.primary.observers_mut()
    }
}
