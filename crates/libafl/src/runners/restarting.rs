use core::time::Duration;

use libafl_core::Error;

use crate::runners::{Runner, RunnerDriver, inprocess::InProcessRunner};

pub struct RestartingRunner<CH, D, S, T, TH> {
    inner: InProcessRunner<CH, D, S, T, TH>,
}

impl<CH, D, S, T, TH> Runner<S> for RestartingRunner<CH, D, S, T, TH>
where
    T: FnOnce(&mut RunnerDriver<S>, &mut S) -> Result<(), Error>,
{
    // TODO: handle fork, state snapshot restore
    unsafe fn run_impl(
        &mut self,
        driver: &mut RunnerDriver<S>,
        state: &mut S,
    ) -> Result<(), Error> {
        self.inner.run_task(driver, state)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.inner.set_timeout(timeout)
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        self.inner.unser_timeout()
    }
}
