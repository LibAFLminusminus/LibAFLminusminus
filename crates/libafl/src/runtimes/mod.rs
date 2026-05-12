//! Module defining [`Runtime`]s.

use crate::{
    DependencyResolver, Fuzzer, Result,
    executors::Executor,
    inputs::Input,
    runtimes::{
        inprocess::{CrashStatus, TimeoutStatus},
        restarting::LIBAFL_EXIT_END,
        utils::{PinnedPtr, unix::OsShmSender},
    },
    stages::StagesTuple,
};
use core::{pin::Pin, ptr::NonNull, time::Duration};
use std::process::exit;

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
    /// This function is only useful for trait writers to implement their custom [`runtime`].
    ///
    /// # Safety
    ///
    /// The `rt_handle` MUST be linked to the current runtime.
    /// Using a `rt_handle` that is not instantiated with self as the runtime will lead to Undefined Behaviour.
    /// Use [`Self::run`], this function should not need to be called directly.
    unsafe fn run_impl(self, state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()>;

    /// Run the runtime.
    fn run(mut self, state: S, worker: W) -> Result<()>
    where
        Self: Sized + 'static,
    {
        let mut rt_handle = unsafe {
            RuntimeHandle::new(
                core::ptr::from_mut::<Self>(&mut self) as *mut dyn Runtime<S, W>,
                worker,
            )
        };

        unsafe { self.run_impl(state, &mut rt_handle)? };

        exit(LIBAFL_EXIT_END);
    }

    /// Set a timeout value for the runtime.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
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

/// Object enabling interacting with a runtime's environment from the task.
/// It can be used to perform runtime-level operations generically.
///
/// It does not expose the runtime directly
#[derive(Debug)]
pub struct RuntimeHandle<S, W> {
    runtime: NonNull<dyn Runtime<S, W>>,
    worker: W,
    termination_data_ptr: Option<PinnedPtr<TerminationHandlerData>>,
    state_shm_sender: Option<OsShmSender<S>>,
}

impl<S, W> RuntimeHandle<S, W> {
    unsafe fn new(runtime: *mut dyn Runtime<S, W>, worker: W) -> Self {
        Self {
            runtime: NonNull::new(runtime).expect("runtime ptr must be non-null"),
            worker,
            termination_data_ptr: None,
            state_shm_sender: None,
        }
    }

    unsafe fn runtime(&self) -> &dyn Runtime<S, W> {
        unsafe { self.runtime.as_ref() }
    }

    unsafe fn runtime_mut(&mut self) -> &mut dyn Runtime<S, W> {
        unsafe { self.runtime.as_mut() }
    }

    /// Set a timeout value for the runtime.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        unsafe { self.runtime_mut().set_timeout(timeout) }
    }

    /// Arm the [`Runtime`]'s timeout.
    pub fn arm_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().arm_timeout() }
    }

    /// Disarm the [`Runtime`]'s timeout.
    pub fn disarm_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().disarm_timeout() }
    }

    /// Unset a previously set timeout.
    ///
    /// If no timeout has been set before, it's a no-op.
    pub fn unset_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().unset_timeout() }
    }

    /// Set the termination handler (used by the [`InProcessRuntime`]).
    ///
    /// # Safety
    ///
    /// `termination_data` must outlive this [`RuntimeHandle`].
    pub unsafe fn set_termination_handler(
        &mut self,
        termination_data: Pin<&mut TerminationHandlerData>,
    ) {
        assert!(
            self.termination_data_ptr.is_none(),
            "Termination data pointer has already been set. This is a fuzzer bug."
        );

        self.termination_data_ptr = Some(unsafe { PinnedPtr::from_pin(termination_data) });
    }

    /// Set the shared memory saver (used by the [`RestartingRuntime`]).
    pub fn set_saver(&mut self, state_shm_sender: OsShmSender<S>) {
        assert!(
            self.state_shm_sender.is_none(),
            "A state shm sender is already set in the runtime handle. This is a fuzzer bug."
        );

        self.state_shm_sender = Some(state_shm_sender);
    }

    /// Initialize the termination global data and handlers.
    pub fn init_termination_handlers<E, I, R, ST, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        executor: &mut E,
        on_crash: fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<CrashStatus>,
        on_timeout: fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<TimeoutStatus>,
    ) where
        E: Executor<I, S>,
        I: Input,
        R: Runtime<S, W>,
        ST: StagesTuple<E, R, S, W, Z>,
        Z: Fuzzer<E, I, R, S, ST, W>,
    {
        let rt_handle_ptr = NonNull::from_mut(self);

        if let Some(termination_data) = self.termination_data_ptr.as_mut() {
            termination_data.init(state, fuzzer, executor, rt_handle_ptr, on_crash, on_timeout);

            if let Some(ref mut saver) = self.state_shm_sender {
                termination_data.set_saver_ptr(saver);
            }
        }
    }

    /// Set the input being run.
    pub fn set_input<I>(&mut self, input: &I) {
        if let Some(signal_data) = self.termination_data_ptr.as_mut() {
            signal_data.set_input(input);
        }
    }

    /// Clear the input being run.
    pub fn clear_input(&mut self) {
        if let Some(signal_data) = self.termination_data_ptr.as_mut() {
            signal_data.clear_input();
        }
    }

    /// Get a reference to the [`Worker`].
    pub fn worker(&self) -> &W {
        &self.worker
    }

    /// Get a mutable reference to the [`Worker`].
    pub fn worker_mut(&mut self) -> &mut W {
        &mut self.worker
    }
}

impl<S, W> DependencyResolver for RuntimeHandle<S, W> {
    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<()> {
        unsafe { self.runtime().check(checker) }
    }

    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        unsafe { self.runtime_mut().register(registrator) }
    }
}
