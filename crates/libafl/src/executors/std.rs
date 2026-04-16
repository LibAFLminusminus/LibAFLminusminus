use core::{marker::PhantomData, time::Duration};
use libafl_core::Error;

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
    _phantom: PhantomData<(C, I, S)>,
}

impl<C, H, I, OT, S> Executor<C, I, OT, S> for StdExecutor<C, H, I, OT, S>
where
    H: FnMut(&mut S, &I) -> Result<ExitKind, Error>,
    OT: ObserversTuple<S>,
{
    fn init(&mut self, driver: &mut RuntimeHandle<C, S>, _controller: &mut C) -> Result<(), Error> {
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

    fn observers_tuple(&self) -> &OT {
        &self.observers
    }

    fn observers_tuple_mut(&mut self) -> &mut OT {
        &mut self.observers
    }
}
