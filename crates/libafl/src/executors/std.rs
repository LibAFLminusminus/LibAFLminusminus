use core::{marker::PhantomData, time::Duration};

use libafl_core::Error;
use tuple_list_ex::RefIndexable;

use crate::{
    CompatibilityChecker, Controller, DependencyResolver, Registrator,
    executors::{Executor, ExitKind},
    observers::{Observer, ObserversTuple},
    runtimes::RuntimeHandle,
};

pub struct StdExecutor<H, I, O, S> {
    harness: H,
    observers: O,
    timeout: Option<Duration>,
    initialized: bool,
    phantom: PhantomData<(I, S)>,
}
impl<H, I, O, S> StdExecutor<H, I, O, S> {
    pub fn new(harness: H, observers: O, timeout: Option<Duration>) -> Self {
        Self {
            harness,
            observers,
            timeout,
            initialized: false,
            phantom: PhantomData,
        }
    }
}

impl<H, I, O, S> DependencyResolver for StdExecutor<H, I, O, S>
where
    O: Observer<S>,
{
    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        registrator.register_ty::<Self>();

        self.register(registrator)?;
        self.observers.register_with_ty(registrator)
    }

    fn check(&self, _checker: &CompatibilityChecker) -> Result<(), Error> {
        Ok(())
    }
}

impl<H, I, O, S> Executor<I, S> for StdExecutor<H, I, O, S>
where
    H: FnMut(&mut S, &I) -> Result<ExitKind, Error>,
    O: Observer<S>,
{
    type Observers = O;

    fn init<CT: Controller>(
        &mut self,
        rt_handle: &mut RuntimeHandle<CT, S>,
        _controller: &mut CT,
    ) -> Result<(), Error> {
        if !self.initialized {
            if let Some(tmout) = &self.timeout {
                rt_handle.set_timeout(tmout.clone());
            }

            self.initialized = true;
        }

        Ok(())
    }

    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind, Error> {
        debug_assert!(self.initialized);

        (self.harness)(state, input)
    }

    fn observers(&self) -> tuple_list_ex::RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(
        &mut self,
    ) -> tuple_list_ex::RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}
