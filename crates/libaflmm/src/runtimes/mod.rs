//! Module defining [`Runtime`]s.

use crate::{Result, common::DependencyResolver, runtimes::restarting::LIBAFLMM_EXIT_END};
use core::time::Duration;
use std::process::exit;

pub mod handle;
pub use handle::RuntimeHandle;

pub mod inprocess;
pub use inprocess::{InProcessRuntime, SimpleInProcessRuntime};

pub mod restarting;
pub use restarting::RestartingRuntime;

pub mod nop;
pub use nop::NopRuntime;

pub mod simple;
pub use simple::SimpleRuntime;

pub mod utils;
pub use utils::{
    IntoTerminationHandlerData, OsTerminationHandler, OsTerminationParams, TerminationHandler,
    TerminationHandlerData,
};

/// The standard forkserver [`Runtime`].
pub type StdForkserverRuntime<T> = SimpleRuntime<T>;

/// The standard in-process [`Runtime`].
pub type StdInProcessRuntime<S, T, TM> = RestartingRuntime<SimpleInProcessRuntime<S, T, TM>>;

/// Environment used to run a task
pub trait Runtime<S, W>: DependencyResolver {
    /// Run the runtime.
    /// A runtime task is terminal: it is called only once and the runtime will immediately exit when the task returns.
    ///
    /// This trait function should NEVER be called by a user directly.
    /// The user is intended to use `run`, as it will always perform the right action.
    ///
    /// This function is only useful for trait writers to implement their custom [`Runtime`].
    ///
    /// # Safety
    ///
    /// The `rt_handle` MUST be linked to the current runtime.
    /// Using a `rt_handle` that is not instantiated with self as the runtime will lead to Undefined Behaviour.
    /// Use [`Self::run`], this function should not need to be called directly.
    unsafe fn run_impl(&mut self, state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()>;

    /// Run the runtime.
    fn run(&mut self, state: S, worker: W) -> Result<()>
    where
        Self: Sized + 'static,
    {
        let mut rt_handle = unsafe {
            RuntimeHandle::new(
                core::ptr::from_mut::<Self>(self) as *mut dyn Runtime<S, W>,
                worker,
            )
        };

        unsafe { self.run_impl(state, &mut rt_handle)? };

        exit(LIBAFLMM_EXIT_END);
    }

    /// Set a timeout value for the runtime.
    fn set_timeout(&mut self, _timeout: Duration) -> Result<()> {
        Ok(())
    }

    /// Arm the timer, with the value previously provided to `set_timeout`
    ///
    /// If no timeout has been set previously, it's a no-op.
    fn arm_timeout(&mut self) -> Result<()> {
        Ok(())
    }

    /// Disarm the timer if it has been previously armed with `arm_timeout`.
    ///
    /// If not timer has been armed previously, it's a no-op.
    fn disarm_timeout(&mut self) -> Result<()> {
        Ok(())
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<()> {
        Ok(())
    }
}
