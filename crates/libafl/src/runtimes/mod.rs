use core::{ptr::NonNull, time::Duration};

use libafl_bolts::Error;

use crate::DependencyResolver;

pub mod direct;
pub mod inprocess;
#[cfg(not(feature = "remove_me"))]
pub mod restarting;

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
    /// The driver MUST be linked to the current runtime.
    /// Using a `driver` that is not instanciated with self as the runtime will lead to Undefined Behaviour.
    /// Use [`Self::run`], this function should not need to be called directly.
    unsafe fn run_impl(
        &mut self,
        driver: &mut RuntimeHandle<CT, S>,
        controller: &mut CT,
    ) -> Result<(), Error>;

    fn run(&mut self, controller: &mut CT) -> Result<(), Error>
    where
        Self: Sized + 'static,
    {
        let mut driver =
            unsafe { RuntimeHandle::new(self as *mut Self as *mut dyn Runtime<CT, S>) };

        unsafe { self.run_impl(&mut driver, controller) }
    }

    /// Set a timeout value for the runtime.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        unimplemented!("This runtime does not implement timeout")
    }

    /// Arm the timer, with the value previously provided to `set_timeout`
    ///
    /// If no timeout has been set previously, it's a no-op.
    fn arm_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("This runtime does not implement timeout")
    }

    /// Disarm the timer if it has been previously armed with `arm_timeout`.
    ///
    /// If not timer has been armed previously, it's a no-op.
    fn disarm_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("This runtime does not implement timeout")
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("This runtime does not implement timeout")
    }
}

/// Object enabling interacting with a runtime's environment from the task.
/// It can be used to perform runtime-level operations generically.
/// It does not expose the runtime directly
pub struct RuntimeHandle<CT, S> {
    runtime: NonNull<dyn Runtime<CT, S>>,
}

impl<CT, S> RuntimeHandle<CT, S> {
    unsafe fn new(runtime: *mut dyn Runtime<CT, S>) -> Self {
        Self {
            runtime: NonNull::new(runtime).expect("runtime ptr must be non-null"),
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
}

impl<CT, S> DependencyResolver for RuntimeHandle<CT, S> {
    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        unsafe { self.runtime().check(checker) }
    }

    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        unsafe { self.runtime_mut().register(registrator) }
    }
}
