use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle, StdInProcessRuntime},
};
use core::{marker::PhantomData, num::NonZeroUsize, time::Duration};
use libafl_bolts::shm::OsShmBuilder;
use libafl_core::Error;
use nix::{
    sys::{
        mman::{MapFlags, ProtFlags, mmap_anonymous},
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, fork, pipe},
};
use serde::{Deserialize, Serialize};
use std::process::exit;

/// end the restarter. task job is over.
pub const LIBAFL_EXIT_END: i32 = 100;

/// restart the task
pub const LIBAFL_EXIT_CONTINUE: i32 = 101;

/// infinite recursion bug in termination handlers.
pub const LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION: i32 = 102;

#[derive(Debug, Clone)]
pub struct RestartingRuntime<RT> {
    inner: RT,
    // The RAM limit for writing state in a shared memory on crash / timeout
    // A good rule of thumb could be to use system_ram / nb_clients.
    // If your state ever gets that big, there is most likely something wrong anyway.
    state_ram_limit: NonZeroUsize,
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

impl<S, T> RestartingRuntime<StdInProcessRuntime<S, T>>
where
    S: Serialize,
{
    pub fn new(task: T, state_ram_limit: NonZeroUsize) -> Self {
        Self::new_generic(StdInProcessRuntime::new(task), state_ram_limit)
    }
}

impl<RT> RestartingRuntime<RT> {
    pub fn new_generic(runtime: RT, state_ram_limit: NonZeroUsize) -> Self {
        Self {
            inner: runtime,
            state_ram_limit,
        }
    }
}

impl<RT, S, W> Runtime<S, W> for RestartingRuntime<RT>
where
    RT: Runtime<S, W>,
    for<'de> S: Serialize + Deserialize<'de>,
{
    unsafe fn run_impl(
        &mut self,
        mut state: S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<(), Error> {
        let (mut saver, mut restorer) = OsShmBuilder::build(self.state_ram_limit)?;

        loop {
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    // parent code, wait for child to end and eventually restart.

                    state = match waitpid(child, None) {
                        Ok(WaitStatus::Exited(pid, status)) => {
                            eprintln!("Child runtime {pid} exited with status: {status}");

                            // the child exited with some status code, handle it here.
                            match status {
                                LIBAFL_EXIT_END => return Ok(()),
                                LIBAFL_EXIT_CONTINUE => {
                                    // at this point, the child finished and must be restarted with the new state. shm must be loaded with state.
                                    // this must be hit on crash / timeout in the child
                                    unsafe { restorer.receive()? }
                                }
                                LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION => {
                                    return Err(Error::runtime(format!(
                                        "An infinite termination recursion occured in the child process."
                                    )));
                                }
                                0..128 => {
                                    return Err(Error::runtime(format!(
                                        "The child returned with exit code {status}. This means the target stopped without being able to save its state. This is a harness bug."
                                    )));
                                }

                                signal_exit => {
                                    /// the child returned with signal exit code
                                    return Err(Error::runtime(format!(
                                        "The child exited with code: {signal_exit}"
                                    )));
                                }
                            }
                        }
                        Ok(WaitStatus::Signaled(pid, signal, core_dumped)) => {
                            log::info!("Child runtime {pid} exited because of signal: {signal}");

                            panic!("Unexpected signal exit");
                        }
                        Ok(exit_reason) => panic!("Unexpected child exit reason: {exit_reason:?}"),
                        Err(e) => {
                            return Err(Error::runtime(format!("Restarter waitpid failed: {e}")));
                        }
                    };
                }
                Ok(ForkResult::Child) => {
                    // child code, setup the saver and start the runtime.

                    // set the state saver, which should be called by the child on erroneous exit.
                    rt_handle.set_saver(saver);

                    self.inner
                        .run_impl(state, rt_handle)
                        .expect("Error while running the child runtime");

                    exit(LIBAFL_EXIT_END);
                }
                Err(e) => {
                    return Err(Error::runtime(format!(
                        "Restarting runtime error while forking: {e}"
                    )));
                }
            };
        }
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
