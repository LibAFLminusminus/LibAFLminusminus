use core::{marker::PhantomData, time::Duration};
use libafl_core::Error;
use tuple_list_ex::RefIndexable;

use crate::{
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
};

pub struct StdExecutor<C, H, I, OT, S> {
    harness: H,
    observers: OT,
    timeout: Option<Duration>,
    initialized: bool,
    phantom: PhantomData<(C, I, S)>,
}
impl<C, H, I, OT, S> StdExecutor<C, H, I, OT, S> {
    pub fn new(harness: H, observers: OT, timeout: Option<Duration>) -> Self {
        Self {
            harness,
            observers,
            timeout,
            initialized: false,
            phantom: PhantomData,
        }
    }
}

impl<CT, H, I, OT, S> Executor<CT, I, S> for StdExecutor<CT, H, I, OT, S>
where
    H: FnMut(&mut S, &I) -> Result<ExitKind, Error>,
{
    type Observers = OT;

    fn init(
        &mut self,
        driver: &mut RuntimeHandle<CT, S>,
        _controller: &mut CT,
    ) -> Result<(), Error> {
        if !self.initialized {
            if let Some(tmout) = &self.timeout {
                driver.set_timeout(tmout.clone());
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
