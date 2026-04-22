use core::{ffi::c_void, ptr::NonNull, time::Duration};

use libafl_bolts::Error;

use crate::{
    DependencyResolver,
    runtimes::utils::{
        IntoTerminationHandlerData, OsTerminationParams, TerminationHandlerData, unix::OsSaver,
    },
};

pub mod inprocess;
// pub mod restarting;
pub mod simple;
pub mod utils;

/// Environment used to run a task
pub trait Runtime<CT, S>: DependencyResolver {
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
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<CT, S>) -> Result<(), Error>;

    fn run(&mut self, controller: &mut CT) -> Result<(), Error>
    where
        Self: Sized + 'static,
    {
        let mut rt_handle =
            unsafe { RuntimeHandle::new(self as *mut Self as *mut dyn Runtime<CT, S>, controller) };

        unsafe { self.run_impl(&mut rt_handle) }
    }

    /// Set a timeout value for the runtime.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        Ok(())
    }

    /// Arm the timer, with the value previously provided to `set_timeout`
    ///
    /// If no timeout has been set previously, it's a no-op.
    fn arm_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Disarm the timer if it has been previously armed with `arm_timeout`.
    ///
    /// If not timer has been armed previously, it's a no-op.
    fn disarm_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Object enabling interacting with a runtime's environment from the task.
/// It can be used to perform runtime-level operations generically.
///
/// It does not expose the runtime directly
pub struct RuntimeHandle<'a, CT, S> {
    runtime: NonNull<dyn Runtime<CT, S>>,
    controller: &'a mut CT,
    termination_data_ptr: Option<NonNull<TerminationHandlerData>>,
    saver: Option<OsSaver<S>>,
}

impl<'a, CT, S> RuntimeHandle<'a, CT, S> {
    unsafe fn new(runtime: *mut dyn Runtime<CT, S>, controller: &'a mut CT) -> Self {
        Self {
            runtime: NonNull::new(runtime).expect("runtime ptr must be non-null"),
            controller,
            termination_data_ptr: None,
            saver: None,
        }
    }

    unsafe fn runtime(&self) -> &dyn Runtime<CT, S> {
        unsafe { self.runtime.as_ref() }
    }

    unsafe fn runtime_mut(&mut self) -> &mut dyn Runtime<CT, S> {
        unsafe { self.runtime.as_mut() }
    }

    /// Set a timeout value for the runtime.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        unsafe { self.runtime_mut().set_timeout(timeout.clone()) }
    }

    pub fn arm_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runtime_mut().arm_timeout() }
    }

    pub fn disarm_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runtime_mut().disarm_timeout() }
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    pub fn unset_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runtime_mut().unset_timeout() }
    }

    pub unsafe fn set_termination_handler<THD: IntoTerminationHandlerData>(
        &mut self,
        termination_data: &mut THD,
    ) {
        if self.termination_data_ptr.is_some() {
            panic!("Termination data pointer has already been set. This is a Fuzzer bug.");
        }

        self.termination_data_ptr = termination_data.as_signal_handler_data();
    }

    pub fn set_saver(&mut self, saver: OsSaver<S>) {
        if self.saver.is_some() {
            panic!("A saver is already set in the runtime handle. This is a Fuzzer bug.");
        }

        self.saver = Some(saver);
    }

    pub fn init_termination_handlers<O, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        observers: &mut O,
        on_crash: fn(&mut TerminationHandlerData, &OsTerminationParams),
        on_timeout: fn(&mut TerminationHandlerData, &OsTerminationParams),
    ) {
        if let Some(mut signal_data) = self.termination_data_ptr {
            unsafe {
                signal_data
                    .as_mut()
                    .init(state, fuzzer, observers, on_crash, on_timeout);
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
}

impl<'a, CT, S> DependencyResolver for RuntimeHandle<'a, CT, S> {
    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        unsafe { self.runtime().check(checker) }
    }

    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        unsafe { self.runtime_mut().register(registrator) }
    }
}
