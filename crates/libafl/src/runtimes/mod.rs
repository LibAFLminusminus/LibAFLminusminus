use core::{ptr::NonNull, time::Duration};

use libafl_bolts::Error;

use crate::DependencyResolver;

pub mod inprocess;
pub mod restarting;

/// Environment used to run a task
pub trait Runtime<S>: DependencyResolver {
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
    unsafe fn run_impl(&mut self, driver: &mut RuntimeHandle<S>) -> Result<(), Error>;

    fn run(&mut self) -> Result<(), Error>
    where
        Self: Sized + 'static,
    {
        let mut driver = unsafe { RuntimeHandle::new(self as *mut Self as *mut dyn Runtime<S>) };

        unsafe { self.run_impl(&mut driver) }
    }

    // fn on_signal()

    /// Set a timeout value for the runtime.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error>;

    /// Arm the timer, with the value previously provided to `set_timeout`
    ///
    /// If no timeout has been set previously, it's a no-op.
    fn arm_timeout(&mut self) -> Result<(), Error>;

    /// Disarm the timer if it has been previously armed with `arm_timeout`.
    ///
    /// If not timer has been armed previously, it's a no-op.
    fn disarm_timeout(&mut self) -> Result<(), Error>;

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error>;
}

/// Object enabling interacting with a runtime's environment from the task.
/// It can be used to perform runtime-level operations generically.
/// It does not expose the runtime directly
pub struct RuntimeHandle<S> {
    runtime: NonNull<dyn Runtime<S>>,
}

impl<S> RuntimeHandle<S> {
    unsafe fn new(runtime: *mut dyn Runtime<S>) -> Self {
        Self {
            runtime: NonNull::new(runtime).expect("runtime ptr must be non-null"),
        }
    }

    unsafe fn runtime(&self) -> &dyn Runtime<S> {
        unsafe { self.runtime.as_ref() }
    }

    unsafe fn runtime_mut(&mut self) -> &mut dyn Runtime<S> {
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

impl<S> DependencyResolver for RuntimeHandle<S> {
    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        unsafe { self.runtime().check(checker) }
    }

    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        unsafe { self.runtime_mut().register(checker) }
    }
}

/// Simplest runtime, just runs the task.
struct DirectRuntime<S, T> {
    state: S,
    task: T,
}

impl<S, T> Runtime<S> for DirectRuntime<S, T>
where
    T: FnMut(&mut RuntimeHandle<S>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(&mut self, driver: &mut RuntimeHandle<S>) -> Result<(), Error> {
        (self.task)(driver, &mut self.state)
    }

    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }

    fn arm_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }

    fn disarm_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }
}
