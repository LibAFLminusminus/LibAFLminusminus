use core::{ptr::NonNull, time::Duration};

use libafl_bolts::Error;

pub mod inprocess;
pub mod restarting;

/// Environment used to run a task
pub trait Runner<S> {
    /// Run the runner.
    /// A runner task is terminal: it is called only once and the runner will immediately exit when the task returns.
    ///
    /// This trait function should NEVER be called by a user directly.
    /// The user is intended to use `run`, as it will always perform the right action.
    ///
    /// This function is only useful for trait writers to implement their custom [`Runner`].
    ///
    /// # Safety
    ///
    /// The driver MUST be linked to the current runner.
    /// Using a `driver` that is not instanciated with self as the runner will lead to Undefined Behaviour.
    /// Use [`Self::run`], this function should not need to be called directly.
    unsafe fn run_impl(&mut self, driver: &mut RunnerDriver<S>, state: &mut S)
    -> Result<(), Error>;

    fn run(&mut self, state: &mut S) -> Result<(), Error>
    where
        Self: Sized + 'static,
    {
        let mut driver = unsafe { RunnerDriver::new(self as *mut Self as *mut dyn Runner<S>) };

        unsafe { self.run_impl(&mut driver, state) }
    }

    // fn on_signal()

    /// Set a timeout value for the runner.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error>;

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error>;
}

/// Object enabling interacting with a runner's environment from the task.
/// It can be used to perform runner-level operations generically.
/// It does not expose the runner directly
pub struct RunnerDriver<S> {
    runner: NonNull<dyn Runner<S>>,
}

impl<S> RunnerDriver<S> {
    unsafe fn new(runner: *mut dyn Runner<S>) -> Self {
        Self {
            runner: NonNull::new(runner).expect("runner ptr must be non-null"),
        }
    }

    unsafe fn runner(&self) -> &dyn Runner<S> {
        unsafe { self.runner.as_ref() }
    }

    unsafe fn runner_mut(&mut self) -> &mut dyn Runner<S> {
        unsafe { self.runner.as_mut() }
    }

    /// Set a timeout value for the runner.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    pub fn set_timeout(&mut self, timeout: &Duration) -> Result<(), Error> {
        unsafe { self.runner_mut().set_timeout(timeout.clone()) }
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    pub fn unset_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runner_mut().unset_timeout() }
    }
}

/// Simplest Runner, just runs the task.
struct DirectRunner<S, T> {
    state: S,
    task: T,
}

impl<S, T> Runner<S> for DirectRunner<S, T>
where
    T: FnMut(&mut RunnerDriver<S>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(
        &mut self,
        driver: &mut RunnerDriver<S>,
        state: &mut S,
    ) -> Result<(), Error> {
        (self.task)(driver, state)
    }

    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        unimplemented!("The direct runner does not implement timeout")
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runner does not implement timeout")
    }
}
