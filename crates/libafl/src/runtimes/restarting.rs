use crate::{
    DependencyResolver,
    runtimes::{
        Runtime, RuntimeHandle,
        utils::unix::{OsSaver, saver::OsSaveRestoreBuilder},
    },
};
use core::{marker::PhantomData, num::NonZeroUsize, time::Duration};
use libafl_core::Error;
use nix::{
    sys::{
        mman::{MapFlags, ProtFlags, mmap_anonymous},
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, fork, pipe},
};

pub struct RestartingRuntime<RT> {
    inner: RT,
    // The RAM limit for writing state in a shared memory on crash / timeout
    // A good rule of thumb could be to use system_ram / nb_clients.
    // If your state ever gets that big, there is most likely something wrong anyway.
    saving_ram_limit: NonZeroUsize,
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
        let (saver, restorer) = OsSaveRestoreBuilder::build(self.saving_ram_limit);

        loop {
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    // parent code, wait for child to end and eventually restart.

                    match waitpid(child, None) {
                        Ok(WaitStatus::Exited(pid, status)) => {
                            eprintln!("Child runtime {pid} exited with status: {status}");
                            // save
                        }
                    }
                }
                Ok(ForkResult::Child) => {
                    // child code, setup the saver and start the runtime.

                    rt_handle.set_saver(saver);

                    self.inner.run_impl(rt_handle)
                }
                Err(e) => {
                    return Err(Error::runtime(format!(
                        "Restarting runtime error while forking: {e}"
                    )));
                }
            };
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
