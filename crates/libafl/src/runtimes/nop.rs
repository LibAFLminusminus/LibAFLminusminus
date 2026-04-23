use libafl_core::Error;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

/// Simplest runtime, just runs the task.
pub struct NopRuntime;

impl DependencyResolver for NopRuntime {}

impl<CT, S> Runtime<CT, S> for NopRuntime {
    unsafe fn run_impl(
        &mut self,
        mut state: S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) -> Result<(), Error> {
        panic!("NopRuntime does not run")
    }
}
