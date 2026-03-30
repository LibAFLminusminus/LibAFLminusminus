use core::time::Duration;

use libafl_core::Error;

use crate::runners::{Runner, inprocess::InProcessRunner};

pub struct RestartingRunner<CH, S, T, TH> {
    inner: InProcessRunner<CH, S, T, TH>,
}

impl<CH, S, T, TH> Runner<S> for RestartingRunner<CH, S, T, TH>
where
    T: FnOnce(&mut S) -> Result<(), Error>,
{
    // TODO: handle fork, state snapshot restore
    fn run_task(&mut self, state: &mut S) -> Result<(), Error> {
        self.inner.run_task(state)
    }

    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        Ok(())
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
