//! Trivial Constant Executor

use super::{Executor, ExitKind};
use crate::{DependencyResolver, Result, observers::ObserversTuple};
use core::time::Duration;
use libafl_bolts::tuples::RefIndexable;

/// [`NopExecutor`] is an executor that does nothing
pub type NopExecutor = ConstantExecutor<()>;

/// Constant Executor that returns a fixed value. Mostly helpful
/// when you need it to satisfy some bounds like [`crate::fuzzer::NopFuzzer`]
#[derive(Debug)]
pub struct ConstantExecutor<OT = ()> {
    exit: ExitKind,
    tm: Duration,
    ot: OT,
}

impl<OT> ConstantExecutor<OT> {
    /// Construct a [`ConstantExecutor`]
    #[must_use]
    pub fn new(exit: ExitKind, tm: Duration, ot: OT) -> Self {
        Self { exit, tm, ot }
    }
}

impl ConstantExecutor<()> {
    /// Create a new `nop` executor that does nothing.
    #[must_use]
    pub fn nop() -> Self {
        Self::new(ExitKind::Ok, Duration::default(), ())
    }
}

impl ConstantExecutor<()> {
    /// Construct a [`ConstantExecutor`] that always returns Ok
    #[must_use]
    pub fn ok() -> Self {
        Self::new(ExitKind::Ok, Duration::default(), ())
    }

    /// Construct a [`ConstantExecutor`] that always returns Crash
    #[must_use]
    pub fn crash() -> Self {
        Self::new(ExitKind::Crash, Duration::default(), ())
    }
}

impl<OT> DependencyResolver for ConstantExecutor<OT> {}

impl<OT, I, S> Executor<I, S> for ConstantExecutor<OT>
where
    OT: ObserversTuple<S>,
{
    type Observers = OT;

    fn init<CT: crate::Controller>(
        &mut self,
        _state: &mut S,
        _rt_handle: &mut crate::runtimes::RuntimeHandle<CT, S>,
    ) -> Result<()> {
        Ok(())
    }

    unsafe fn execute_impl(&mut self, _state: &mut S, _input: &I) -> Result<ExitKind> {
        Ok(self.exit)
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.ot)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.ot)
    }
}
