use core::{marker::PhantomData, time::Duration};
use libafl_core::Error;

use crate::{
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    runners::RunnerDriver,
    state::State,
};

pub struct StdExecutor<H, I, OT, S> {
    harness: H,
    observers: OT,
    timeout: Option<Duration>,
    initialized: bool,
    _phantom: PhantomData<(I, S)>,
}

impl<H, I, OT, S> Executor<I, OT, S> for StdExecutor<H, I, OT, S>
where
    H: FnMut(&mut S, &I) -> Result<ExitKind, Error>,
    OT: ObserversTuple<S>,
    S: State,
{
    fn init(&mut self, driver: &mut RunnerDriver<S>) -> Result<(), Error> {
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
