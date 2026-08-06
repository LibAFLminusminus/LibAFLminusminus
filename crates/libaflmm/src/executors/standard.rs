//! Standard [`Executor`].

use crate::{
    common::{CompatibilityChecker, DependencyResolver, Registrator},
    controllers::Worker,
    executors::{Executor, ExitKind, hooks::ExecutorHooksTuple},
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
    runtimes::{
        inprocess::{CrashStatus, TimeoutStatus},
        utils::OsTerminationParams,
    },
};
use core::{marker::PhantomData, time::Duration};
use libaflmm_bolts::{tuple_list, tuples::RefIndexable};
use libaflmm_core::Result;

/// A standard [`Executor`], used for casual fuzzing.
#[derive(Debug)]
pub struct StdExecutor<EH, H, I, O, S> {
    hooks: EH,
    harness: H,
    observers: O,
    timeout: Option<Duration>,
    initialized: bool,
    phantom: PhantomData<(I, S)>,
}

impl<EH, H, I, O, S> StdExecutor<EH, H, I, O, S> {
    /// Create a new [`StdExecutor`] with hooks.
    pub fn with_hooks(
        _state: &S,
        hooks: EH,
        harness: H,
        observers: O,
        timeout: Option<Duration>,
    ) -> Self
    where
        H: FnMut(&mut S, &I) -> Result<ExitKind>,
    {
        Self {
            hooks,
            harness,
            observers,
            timeout,
            initialized: false,
            phantom: PhantomData,
        }
    }
}

impl<H, I, O, S> StdExecutor<(), H, I, O, S> {
    /// Create a new [`StdExecutor`].
    pub fn new(state: &S, harness: H, observers: O, timeout: Option<Duration>) -> Self
    where
        H: FnMut(&mut S, &I) -> Result<ExitKind>,
    {
        Self::with_hooks(state, tuple_list!(), harness, observers, timeout)
    }
}

impl<EH, H, I, O, S> DependencyResolver for StdExecutor<EH, H, I, O, S>
where
    O: ObserversTuple<S> + DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.observers.register(registrator)
    }

    fn check(&self, _checker: &CompatibilityChecker) -> Result<()> {
        Ok(())
    }
}

impl<EH, H, I, O, S> Executor<I, S> for StdExecutor<EH, H, I, O, S>
where
    EH: ExecutorHooksTuple<I, S>,
    H: FnMut(&mut S, &I) -> Result<ExitKind>,
    O: ObserversTuple<S> + DependencyResolver,
{
    type Observers = O;

    fn init<W: Worker>(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        if !self.initialized {
            if let Some(tmout) = &self.timeout {
                rt_handle.set_timeout(*tmout)?;
            }

            self.hooks.init_all(state);

            self.initialized = true;
        }

        Ok(())
    }

    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind> {
        debug_assert!(self.initialized);

        self.hooks.pre_exec_all(state, input);

        let res = (self.harness)(state, input)?;

        self.hooks.post_exec_all(state, input);

        Ok(res)
    }

    unsafe fn handle_crash(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        _params: &OsTerminationParams,
    ) -> Result<CrashStatus> {
        if let Some(input) = input {
            self.hooks.post_exec_all(state, input);
        }

        Ok(CrashStatus::TargetCrash)
    }

    unsafe fn handle_timeout(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        _params: &OsTerminationParams,
    ) -> Result<TimeoutStatus> {
        if let Some(input) = input {
            self.hooks.post_exec_all(state, input);
        }

        Ok(TimeoutStatus::Exit)
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}
