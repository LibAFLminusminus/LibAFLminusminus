//! Standard [`Executor`].

use crate::{
    CompatibilityChecker, DependencyResolver, Registrator, Worker,
    executors::{Executor, ExitKind, hooks::ExecutorHooksTuple},
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
};
use core::{marker::PhantomData, time::Duration};
use libafl_core::Result;
use tuple_list::tuple_list;
use tuple_list_ex::RefIndexable;

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
    pub fn with_hooks(hooks: EH, harness: H, observers: O, timeout: Option<Duration>) -> Self {
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
    pub fn new(harness: H, observers: O, timeout: Option<Duration>) -> Self {
        Self::with_hooks(tuple_list!(), harness, observers, timeout)
    }
}

impl<EH, H, I, O, S> DependencyResolver for StdExecutor<EH, H, I, O, S>
where
    O: ObserversTuple<S> + DependencyResolver,
{
    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();

        self.register(registrator)?;
        self.observers.register_with_ty(registrator)
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

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}
