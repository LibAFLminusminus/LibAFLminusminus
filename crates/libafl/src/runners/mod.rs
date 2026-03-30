use core::time::Duration;

use libafl_bolts::Error;

pub mod inprocess;
pub mod restarting;

/// Environment used to run a task
pub trait Runner<S> {
    /// Start the task.
    fn run_task(&mut self, state: &mut S) -> Result<(), Error>;

    /// Set a timeout value for the runner.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error>;

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error>;
}

/// Simplest Runner, just runs the task.
struct DirectRunner<S, T> {
    state: S,
    task: T,
}

impl<S, T> Runner<S> for DirectRunner<S, T>
where
    T: FnOnce(&mut S) -> Result<(), Error>,
{
    fn run_task(&mut self, state: &mut S) -> Result<(), Error> {
        self.task(state)
    }

    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        Ok(())
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
