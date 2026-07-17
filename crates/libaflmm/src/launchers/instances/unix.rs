//! Unix instance

use crate::{
    Error, Result,
    controllers::{Controller, Worker},
    monitors::Monitor,
    runtimes::LIBAFLMM_EXIT_END,
};
use alloc::vec::Vec;
use core::fmt::Debug;
use core::{fmt, time::Duration};
use libaflmm_bolts::core_affinity::CoreId;
use libaflmm_core::runtime;
use nix::{
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::{
        prctl::set_pdeathsig,
        signal::{SigSet, SigmaskHow, Signal, sigprocmask},
        signalfd::{SfdFlags, SignalFd},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::{ForkResult, Pid, fork, getpid, getppid},
};
use std::{collections::HashMap, os::fd::AsFd, process::exit};

/// An Instance ID, unique for each [`Instance`].
pub type InstanceId = u32;

pub type InstanceRunner<W> = Box<dyn FnOnce(W) -> Result<()>>;

/// An instance, owning a running [`Runtime`](crate::runtimes::Runtime).
pub struct Instance<W> {
    runner: InstanceRunner<W>,
    worker: W,
    core: Option<CoreId>,
}

/// [`Instance`] failure reason
#[derive(Debug)]
pub enum InstanceFailure {
    /// Exit due to an unexpected exit code
    Exited(i32),
    /// Exit due to a signal
    Signaled(Signal),
}

/// A collection of [`Instance`]s.
///
/// It should contain all the instances being run.
#[derive(Debug)]
pub struct Instances<D, W> {
    instances: Vec<Instance<W>>,
    active_instances: HashMap<Pid, D>,
}

impl<W> Debug for Instance<W>
where
    W: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instance")
            .field("worker", &self.worker)
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl<W> Instance<W> {
    /// # Safety
    ///
    /// This will spawn a new process, which could have side effects.
    /// Once spawned, the parent process will take back the hand on the control flow immediately.
    pub unsafe fn spawn<CT>(self, controller: &mut CT) -> Result<(Pid, W::Descriptor)>
    where
        CT: Controller<Worker = W>,
        W: Worker,
    {
        // take these out before fork, to mark these as used in the father.
        // the father process will be able to drop the controller in the
        // father process as well.

        let runner = self.runner;
        let core = self.core;
        let mut worker = self.worker;

        let parent_pid = getpid();

        match unsafe { fork()? } {
            ForkResult::Parent { child } => {
                controller
                    .on_worker_start(worker.descriptor(), child.as_raw().try_into().unwrap())?;

                Ok((child, worker.descriptor().clone()))
            }
            ForkResult::Child => {
                set_pdeathsig(Signal::SIGKILL)?;

                if getppid() != parent_pid {
                    // race condition between set_pdeathsig call and parent dying.
                    exit(0);
                }

                if let Some(core_id) = core {
                    core_id.set_affinity()?;
                }

                worker.pre_runtime_exec()?;

                // start the child runtime
                runner(worker)?;

                // TODO: what should we do there in case it happens?
                // i'll panic for now, but it's not the right solution
                panic!("The runtime finished but did not exit cleanly.");
            }
        }
    }
}

impl<D, W> Default for Instances<D, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D, W> Instances<D, W> {
    /// Create a new [`Instances`] collection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            active_instances: HashMap::new(),
        }
    }

    /// Add an [`Instance`] to the collection.
    pub fn add<R>(&mut self, runner: R, worker: W, core: Option<CoreId>)
    where
        R: FnOnce(W) -> Result<()> + 'static,
    {
        self.instances
            .push(Instance::new(Box::new(runner), worker, core));
    }

    /// Whether there are instances or not
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}

impl<D, W> Instances<D, W>
where
    W: Worker,
{
    /// Spawn all [`Instance`]s being owned by [`Self`].
    pub fn spawn_instances<CT>(&mut self, controller: &mut CT) -> Result<()>
    where
        CT: Controller<Worker = W>,
        W: Worker<Descriptor = D>,
    {
        for instance in &mut self.instances.drain(..) {
            let (pid, desc) = unsafe { instance.spawn(controller)? };
            self.active_instances.insert(pid, desc);
        }

        Ok(())
    }

    /// Wait that all [`Instance`]s being owned by [`Self`] end.
    ///
    /// It MUST be run after calling [`Self::spawn_instances`].
    pub fn wait_instances<CT, MT>(
        &mut self,
        controller: &mut CT,
        monitor: &mut MT,
        timeout: Duration,
    ) -> Result<()>
    where
        W: Worker<Descriptor = D>,
        CT: Controller<Worker = W>,
        MT: Monitor,
    {
        let mut sigset = SigSet::empty();
        sigset.add(Signal::SIGCHLD);
        sigprocmask(SigmaskHow::SIG_BLOCK, Some(&sigset), None)
            .map_err(|e| Error::runtime(format!("sigprocmask failed: {e}")))?;

        let sfd = SignalFd::with_flags(&sigset, SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC)
            .map_err(|e| Error::runtime(format!("signalfd failed: {e}")))?;

        let mut failures: Vec<(Pid, InstanceFailure)> = Vec::new();

        // collect children that exited before we set up the signalfd.
        self.drain_children(controller, &mut failures)?;

        let poll_timeout = PollTimeout::try_from(timeout).expect("Incorrect poll timeout");

        while !self.active_instances.is_empty() {
            monitor.display(controller)?;

            let mut fds = [PollFd::new(sfd.as_fd(), PollFlags::POLLIN)];
            match poll(&mut fds, poll_timeout) {
                Err(nix::errno::Errno::EINTR) | Ok(0) => {
                    // Interrupted by signal or timed out; retry.
                }
                Err(e) => return Err(Error::runtime(format!("poll failed: {e}"))),
                Ok(_) => {
                    // consume the pending signals
                    while matches!(sfd.read_signal(), Ok(Some(_))) {}

                    // collect children that exited
                    self.drain_children(controller, &mut failures)?;
                }
            }
        }

        if failures.is_empty() {
            log::info!("All instances finished successfully.");
            Ok(())
        } else {
            Err(runtime!(
                "{} instances ended with errors: {failures:?}",
                failures.len()
            ))
        }
    }

    fn retire(&mut self, pid: Pid) -> D {
        self.active_instances.remove(&pid).unwrap_or_else(|| {
            panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug.")
        })
    }

    /// collect dead children correctly.
    fn drain_children<CT>(
        &mut self,
        controller: &mut CT,
        failures: &mut Vec<(Pid, InstanceFailure)>,
    ) -> Result<()>
    where
        CT: Controller<Worker = W, Descriptor = D>,
        W: Worker<Controller = CT>,
    {
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::ECHILD) => break,
                Ok(WaitStatus::Exited(pid, exit_code)) => {
                    let desc = self.retire(pid);

                    if exit_code == LIBAFLMM_EXIT_END {
                        log::info!("Instance with PID {pid} finished its task correctly.");
                    } else {
                        log::warn!(
                            "Instance with PID {pid} exited unexpectedly with code: {exit_code}."
                        );
                        failures.push((pid, InstanceFailure::Exited(exit_code)));
                    }

                    controller.on_worker_exit(&desc, exit_code)?;
                }
                Ok(WaitStatus::Signaled(pid, signal, _)) => {
                    let desc = self.retire(pid);

                    log::warn!("Instance with PID {pid} exited because of signal {signal}");
                    failures.push((pid, InstanceFailure::Signaled(signal)));

                    controller.on_worker_termination(&desc, signal)?;
                }
                Ok(_) => {}
                Err(e) => return Err(runtime!("waitpid failed: {e}")),
            }
        }
        Ok(())
    }
}

impl<W> Instance<W> {
    /// Create a new instance.
    pub fn new(runner: InstanceRunner<W>, worker: W, core: Option<CoreId>) -> Self {
        Self {
            runner,
            worker,
            core,
        }
    }
}
