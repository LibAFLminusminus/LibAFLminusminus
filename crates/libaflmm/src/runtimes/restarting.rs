//! The module for the [`RestartingRuntime`].

use core::{num::NonZeroUsize, time::Duration};
use std::process::exit;

use libaflmm_core::{Error, Result};
use nix::{
    sys::{
        prctl::set_pdeathsig,
        signal::Signal,
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, fork, getpid, getppid},
};
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle, SimpleInProcessRuntime, utils::unix::OsShmBuilder},
};

/// End the restarter; the task is over.
pub const LIBAFL_EXIT_END: i32 = 100;

/// Restart the task
pub const LIBAFL_EXIT_RESTART: i32 = 101;

/// Infinite recursion bug in termination handlers.
pub const LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION: i32 = 102;

/// A restarting [`Runtime`].
///
/// The inner runtime will restart when it exits with special exit codes:
///     - [`LIBAFL_EXIT_END`]: The runtime finished its task, exit successfully.
///     - [`LIBAFL_EXIT_RESTART`]: The runtime must be restarted but no hard error happened.
///     - [`LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION`]: The runtime signal handler is in an infinite recursion. It's a bug.
#[derive(Debug, Clone)]
pub struct RestartingRuntime<RT> {
    inner: RT,
    // The RAM limit for writing state in a shared memory on crash / timeout
    // A good rule of thumb could be to use system_ram / nb_clients.
    // If your state ever gets that big, there is most likely something wrong anyway.
    state_ram_limit: NonZeroUsize,
    // the runtime timeout
    timeout: Option<Duration>,
}

impl<RT> DependencyResolver for RestartingRuntime<RT>
where
    RT: DependencyResolver,
{
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        self.inner.register(registrator)
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<()> {
        self.inner.check(checker)
    }
}

impl<S, T, TM> RestartingRuntime<SimpleInProcessRuntime<S, T, TM>>
where
    S: Serialize,
{
    /// Create a new [`RestartingRuntime`] with the [`SimpleInProcessRuntime`].
    pub fn new(
        task: T,
        state_ram_limit: NonZeroUsize,
        timer: TM,
        timeout: Option<Duration>,
    ) -> Self {
        Self::new_generic(
            SimpleInProcessRuntime::new(task, timer),
            state_ram_limit,
            timeout,
        )
    }
}

impl<RT> RestartingRuntime<RT> {
    /// Create a new [`RestartingRuntime`] with the given [`Runtime`].
    pub fn new_generic(
        runtime: RT,
        state_ram_limit: NonZeroUsize,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            inner: runtime,
            state_ram_limit,
            timeout,
        }
    }
}

impl<RT, S, W> Runtime<S, W> for RestartingRuntime<RT>
where
    RT: Runtime<S, W>,
    for<'de> S: Serialize + Deserialize<'de>,
{
    unsafe fn run_impl(mut self, mut state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()> {
        let (state_sender, mut state_receiver) =
            OsShmBuilder::build_with_hdr::<usize, S>(self.state_ram_limit.get())?;

        let parent_pid = getpid();

        loop {
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    // parent code, wait for child to end and eventually restart.

                    state = match waitpid(child, None) {
                        Ok(WaitStatus::Exited(pid, status)) => {
                            log::info!("Child runtime {pid} exited with status: {status}");

                            // the child exited with some status code, handle it here.
                            match status {
                                LIBAFL_EXIT_END => return Ok(()),
                                LIBAFL_EXIT_RESTART => {
                                    // at this point, the child finished and must be restarted with the new state. shm must be loaded with state.
                                    // this must be hit on crash / timeout in the child
                                    unsafe { state_receiver.receive()? }
                                }
                                LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION => {
                                    return Err(Error::runtime(
                                        "An infinite termination recursion occured in the child process.",
                                    ));
                                }
                                0..128 => {
                                    return Err(Error::runtime(format!(
                                        "The child returned with exit code {status}. This means the target stopped without being able to save its state. This is a harness bug."
                                    )));
                                }

                                signal_exit => {
                                    // the child returned with signal exit code
                                    return Err(Error::runtime(format!(
                                        "The child exited with code: {signal_exit}"
                                    )));
                                }
                            }
                        }
                        Ok(WaitStatus::Signaled(pid, signal, _core_dumped)) => {
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
                    // child stop on father death.
                    set_pdeathsig(Signal::SIGKILL)?;

                    if getppid() != parent_pid {
                        // handle racey call to set_pdeathsig
                        exit(LIBAFL_EXIT_END);
                    }

                    // set the state saver, which should be called by the child on erroneous exit.
                    rt_handle.set_saver(state_sender);

                    // we are in the final process, we can set the timeout now
                    if let Some(timeout) = self.timeout.take() {
                        self.set_timeout(timeout)?;
                    }

                    unsafe {
                        self.inner
                            .run_impl(state, rt_handle)
                            .expect("Error while running the child runtime");
                    }

                    exit(LIBAFL_EXIT_END);
                }
                Err(e) => {
                    return Err(Error::runtime(format!(
                        "Restarting runtime error while forking: {e}"
                    )));
                }
            }
        }
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.inner.set_timeout(timeout)
    }

    fn arm_timeout(&mut self) -> Result<()> {
        self.inner.arm_timeout()
    }

    fn disarm_timeout(&mut self) -> Result<()> {
        self.inner.disarm_timeout()
    }

    fn unset_timeout(&mut self) -> Result<()> {
        self.inner.unset_timeout()
    }
}
