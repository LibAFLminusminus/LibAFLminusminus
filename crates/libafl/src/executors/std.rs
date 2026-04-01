use core::{marker::PhantomData, time::Duration};

use libafl_core::Error;
use tuple_list_ex::RefIndexable;

use crate::{
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    state::State,
};

pub struct StdExecutor<H, I, OT, R, S> {
    harness: H,
    observers: OT,
    timeout: Option<Duration>,
    _phantom: PhantomData<(I, R, S)>,
}

impl<H, I, OT, R, S> Executor<I, OT, R, S> for StdExecutor<H, I, OT, R, S>
where
    OT: ObserversTuple<S>,
    S: State<I>,
{
    fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind, Error> {
        todo!()
    }

    fn observers(&self) -> RefIndexable<&OT, OT> {
        todo!()
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut OT, OT> {
        todo!()
    }

    fn timeout(&self) -> Option<&Duration> {
        self.timeout.as_ref()
    }
}
