use core::time::Duration;

use libafl_core::Error;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

/// Simplest runtime, just runs the task.
pub struct DirectRuntime<S, T> {
    state: S,
    task: T,
}

impl<S, T> DirectRuntime<S, T> {
    pub fn new(state: S, task: T) -> Self {
        Self { state, task }
    }
}

impl<S, T> DependencyResolver for DirectRuntime<S, T> {}

impl<CT, S, T> Runtime<CT, S> for DirectRuntime<S, T>
where
    T: FnMut(&mut RuntimeHandle<CT, S>, &mut S, &mut CT) -> Result<(), Error>,
{
    unsafe fn run_impl(
        &mut self,
        driver: &mut RuntimeHandle<CT, S>,
        controller: &mut CT,
    ) -> Result<(), Error> {
        (self.task)(driver, &mut self.state, controller)
    }
}
