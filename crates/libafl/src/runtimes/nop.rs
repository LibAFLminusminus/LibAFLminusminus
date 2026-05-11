//! The module for the [`NopRuntime`].

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};
use libafl_core::Result;

/// Simplest runtime, just runs the task.
#[derive(Debug)]
pub struct NopRuntime;

impl DependencyResolver for NopRuntime {}

impl<S, W> Runtime<S, W> for NopRuntime {
    unsafe fn run_impl(&mut self, _state: S, _rt_handle: &mut RuntimeHandle<S, W>) -> Result<()> {
        panic!("NopRuntime does not run")
    }
}
