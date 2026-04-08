use core::{marker::PhantomData, time::Duration};
use libafl_core::Error;

use crate::{
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    state::State,
};

pub struct StdExecutor<H, I, OT, S> {
    harness: H,
    observers: OT,
    timeout: Option<Duration>,
    _phantom: PhantomData<(I, S)>,
}

impl<H, I, OT, S> Executor<I, OT, S> for StdExecutor<H, I, OT, S>
where
    H: FnMut(&mut S, &I) -> Result<ExitKind, Error>,
    OT: ObserversTuple<S>,
    S: State<I>,
{
    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind, Error> {
        (self.harness)(state, input)
    }

    fn observers_tuple(&self) -> &OT {
        &self.observers
    }

    fn observers_tuple_mut(&mut self) -> &mut OT {
        &mut self.observers
    }

    fn timeout(&self) -> Option<&Duration> {
        self.timeout.as_ref()
    }
}
