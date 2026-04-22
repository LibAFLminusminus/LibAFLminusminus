use core::time::Duration;

use libafl_core::Error;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

/// Simplest runtime, just runs the task.
pub struct SimpleRuntime<T> {
    task: T,
}

impl<T> SimpleRuntime<T> {
    pub fn new(task: T) -> Self {
        Self { task }
    }
}

impl<T> DependencyResolver for SimpleRuntime<T> {}

impl<CT, S, T> Runtime<CT, S> for SimpleRuntime<T>
where
    T: FnMut(&mut RuntimeHandle<CT, S>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(
        &mut self,
        mut state: S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) -> Result<(), Error> {
        (self.task)(rt_handle, &mut state)
    }
}
