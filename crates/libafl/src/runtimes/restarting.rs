use core::time::Duration;

use libafl_core::Error;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle, inprocess::InProcessRuntime},
};

pub struct RestartingRuntime<CH, D, S, T, TH> {
    inner: InProcessRuntime<CH, D, S, T, TH>,
}

impl<CH, D, S, T, TH> DependencyResolver for RestartingRuntime<CH, D, S, T, TH> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.inner.register(registrator)
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        self.inner.check(checker)
    }
}

impl<CH, D, S, T, TH> Runtime<S> for RestartingRuntime<CH, D, S, T, TH>
where
    T: FnMut(&mut RuntimeHandle<S>, &mut S) -> Result<(), Error>,
    CH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    D: Send + Sync + Unpin + 'static,
    TH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
{
    // TODO: handle fork, state snapshot restore
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<S>) -> Result<(), Error> {
        unsafe { self.inner.run_impl(rt_handle) }
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
