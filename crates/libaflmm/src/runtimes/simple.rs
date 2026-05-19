//! The module for the [`SimpleRuntime`].

use libaflmm_core::Result;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

/// Simplest runtime, just runs the task.
#[derive(Clone, Debug)]
pub struct SimpleRuntime<T> {
    task: T,
}

impl<T> SimpleRuntime<T> {
    /// Create a new [`SimpleRuntime`]
    pub fn new(task: T) -> Self {
        Self { task }
    }
}

impl<T> DependencyResolver for SimpleRuntime<T> {}

impl<S, T, W> Runtime<S, W> for SimpleRuntime<T>
where
    T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()>,
{
    unsafe fn run_impl(mut self, mut state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()> {
        (self.task)(rt_handle, &mut state)
    }
}
