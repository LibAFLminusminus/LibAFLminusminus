use core::{ffi::c_void, ptr::NonNull, time::Duration};
use std::process::exit;

use libafl_bolts::Error;

use crate::{
    DependencyResolver, Result,
    runtimes::{
        restarting::LIBAFL_EXIT_END,
        utils::{
            IntoTerminationHandlerData, OsTerminationParams, TerminationHandlerData,
            unix::OsShmSender,
        },
    },
};

pub mod inprocess;
pub use inprocess::{InProcessRuntime, StdInProcessRuntime};
pub mod restarting;
pub use restarting::RestartingRuntime;
pub mod nop;
pub mod simple;
pub mod utils;

pub type StdRuntime<S, T> = RestartingRuntime<StdInProcessRuntime<S, T>>;

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
    /// The rt_handle MUST be linked to the current runtime.
    /// Using a `rt_handle` that is not instanciated with self as the runtime will lead to Undefined Behaviour.
    /// Use [`Self::run`], this function should not need to be called directly.
    unsafe fn run_impl(&mut self, state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()>;

    fn run(&mut self, state: S, worker: W) -> Result<()>
    where
        Self: Sized + 'static,
    {
        let mut rt_handle =
            unsafe { RuntimeHandle::new(self as *mut Self as *mut dyn Runtime<S, W>, worker) };

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
pub struct RuntimeHandle<S, W> {
    runtime: NonNull<dyn Runtime<S, W>>,
    worker: W,
    termination_data_ptr: Option<NonNull<TerminationHandlerData>>,
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
        unsafe { self.runtime_mut().set_timeout(timeout.clone()) }
    }

    pub fn arm_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().arm_timeout() }
    }

    pub fn disarm_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().disarm_timeout() }
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    pub fn unset_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().unset_timeout() }
    }

    pub unsafe fn set_termination_handler<THD: IntoTerminationHandlerData>(
        &mut self,
        termination_data: &mut THD,
    ) {
        if self.termination_data_ptr.is_some() {
            panic!("Termination data pointer has already been set. This is a fuzzer bug.");
        }

        self.termination_data_ptr = termination_data.as_termination_handler_data();
    }

    pub fn set_saver(&mut self, state_shm_sender: OsShmSender<S>) {
        if self.state_shm_sender.is_some() {
            panic!(
                "A state shm sender is already set in the runtime handle. This is a fuzzer bug."
            );
        }

        self.state_shm_sender = Some(state_shm_sender);
    }

    pub fn init_termination_handlers<O, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        observers: &mut O,
        on_crash: fn(&mut TerminationHandlerData, &OsTerminationParams),
        on_timeout: fn(&mut TerminationHandlerData, &OsTerminationParams),
    ) {
        let rt_handle_ptr = NonNull::from_mut(self);

        if let Some(mut termination_data) = self.termination_data_ptr {
            unsafe {
                termination_data.as_mut().init(
                    state,
                    fuzzer,
                    observers,
                    rt_handle_ptr,
                    on_crash,
                    on_timeout,
                );

                if let Some(ref mut saver) = self.state_shm_sender {
                    termination_data.as_mut().set_saver_ptr(saver);
                }
            }
        }
    }

    pub fn set_input<I>(&mut self, input: &I) {
        if let Some(mut signal_data) = self.termination_data_ptr {
            unsafe {
                signal_data.as_mut().set_input(input);
            }
        }
    }

    pub fn clear_input(&mut self) {
        if let Some(mut signal_data) = self.termination_data_ptr {
            unsafe {
                signal_data.as_mut().clear_input();
            }
        }
    }

    pub fn worker(&self) -> &W {
        &self.worker
    }

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
