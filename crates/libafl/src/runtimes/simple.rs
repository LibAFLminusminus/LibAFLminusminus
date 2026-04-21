use core::time::Duration;

use libafl_core::Error;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

/// Simplest runtime, just runs the task.
pub struct SimpleRuntime<S, T> {
    state: S,
    task: T,
}

impl<S, T> SimpleRuntime<S, T> {
    pub fn new(state: S, task: T) -> Self {
        Self { state, task }
    }
}

impl<S, T> DependencyResolver for SimpleRuntime<S, T> {}

impl<CT, S, T> Runtime<CT, S> for SimpleRuntime<S, T>
where
    T: FnMut(&mut RuntimeHandle<CT, S>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<CT, S>) -> Result<(), Error> {
        (self.task)(rt_handle, &mut self.state)
    }
}
