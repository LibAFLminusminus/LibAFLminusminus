use core::{marker::PhantomData, time::Duration};

use libafl_core::Error;
use nix::{
    sys::wait::{WaitStatus, waitpid},
    unistd::{ForkResult, fork},
};

use crate::{
    DependencyResolver,
    runtimes::{IntoSignalHandlerData, Runtime, RuntimeHandle, inprocess::InProcessRuntime},
};

pub struct RestartingRuntime<RT> {
    inner: RT,
}

impl<RT> DependencyResolver for RestartingRuntime<RT>
where
    RT: DependencyResolver,
{
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.inner.register(registrator)
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        self.inner.check(checker)
    }
}

impl<CT, RT, S> Runtime<CT, S> for RestartingRuntime<RT>
where
    RT: Runtime<CT, S>,
{
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<CT, S>) -> Result<(), Error> {
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => match waitpid(child, None) {
                Ok(WaitStatus::Exited(pid, status)) => {
                    eprintln!("Child runtime {pid} exited with status: {status}");
                    // save
                }
            },
            Ok(ForkResult::Child) => {
                // running the child runtime here
                self.inner.run_impl(rt_handle)
            }
            Err(e) => {
                return Err(Error::runtime(format!(
                    "Restarting runtime error while forking: {e}"
                )));
            }
        }

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
