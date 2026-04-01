use core::{marker::PhantomData, pin::Pin, ptr::NonNull, time::Duration};

use libafl_bolts::Error;

pub mod inprocess;
pub mod restarting;

/// Environment used to run a task
pub trait Runner<S> {
    /// Start the task.
    /// A runner task is terminal: it is called only once and the runner will immediately exit when the task returns.
    ///
    /// # Safety
    ///
    /// The driver MUST be linked to the current runner.
    unsafe fn run_task_impl(
        &mut self,
        driver: Pin<&mut RunnerDriver<Self, S>>,
        state: &mut S,
    ) -> Result<(), Error>;

    fn run_task(&mut self, state: &mut S) -> Result<(), Error> {
        let mut driver = self.create_driver()?;

        let pinned_driver = Pin::new(&mut driver);

        // sigsetjmp

        unsafe { self.run_task_impl(pinned_driver, state) }
    }

    fn on_signal()

    /// Set a timeout value for the runner.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error>;

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error>;

    fn creat_driver(&mut self) -> RunnerDriver<Self, S>;
}

/// Object enabling interacting with a runner's environment from the task.
/// It can be used to perform runner-level operations generically.
/// It does not expose the runner directly
pub struct RunnerDriver<R, S> {
    runner: NonNull<R>,
    _marker: PhantomData<S>,
}

impl<R, S> RunnerDriver<R, S>
where
    R: Runner<S>,
{
    unsafe fn runner(&self) -> &R {
        unsafe { self.runner.as_ref() }
    }

    unsafe fn runner_mut(&mut self) -> &mut R {
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
    T: FnOnce(&mut RunnerDriver<Self, S>, &mut S) -> Result<(), Error>,
{
    fn run_task(
        &mut self,
        driver: Pin<&mut RunnerDriver<Self, S>>,
        state: &mut S,
    ) -> Result<(), Error> {
        self.task(state)
    }

    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        unimplemented!("The direct runner does not implement timeout")
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runner does not implement timeout")
    }
}
