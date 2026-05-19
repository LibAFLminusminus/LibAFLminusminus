//! The module for the [`NopRuntime`].

use libaflmm_core::Result;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

/// Simplest runtime, just runs the task.
#[derive(Debug)]
pub struct NopRuntime;

impl DependencyResolver for NopRuntime {}

impl<S, W> Runtime<S, W> for NopRuntime {
    unsafe fn run_impl(self, _state: S, _rt_handle: &mut RuntimeHandle<S, W>) -> Result<()> {
        panic!("NopRuntime does not run")
    }
}
