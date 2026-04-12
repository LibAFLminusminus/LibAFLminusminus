use core::time::Duration;

use libafl_core::Error;

use crate::runners::{Runner, RunnerDriver, inprocess::InProcessRunner};

pub struct RestartingRunner<CH, D, S, T, TH> {
    inner: InProcessRunner<CH, D, S, T, TH>,
}

impl<CH, D, S, T, TH> Runner<S> for RestartingRunner<CH, D, S, T, TH>
where
    T: FnMut(&mut RunnerDriver<S>, &mut S) -> Result<(), Error>,
    CH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    D: Send + Sync + Unpin + 'static,
    TH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
{
    // TODO: handle fork, state snapshot restore
    unsafe fn run_impl(&mut self, driver: &mut RunnerDriver<S>) -> Result<(), Error> {
        self.inner.run_impl(driver)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.inner.set_timeout(timeout)
    }

    fn arm_timeout(&mut self) -> Result<(), Error> {
        self.inner.arm_timeout()
    }

    fn disarm_timeout(&mut self) -> Result<(), Error> {
        self.inner.disarm_timeout()
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        self.inner.unset_timeout()
    }
}
