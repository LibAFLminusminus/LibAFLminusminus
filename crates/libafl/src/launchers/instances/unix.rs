//! Unix instance

use core::{borrow::Borrow, hash::Hash, time::Duration};
use alloc::vec::Vec;
use std::{collections::HashSet, os::fd::AsFd, process::exit};

use libafl_bolts::core_affinity::CoreId;
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

use crate::{Controller, Error, Result, Worker, monitors::Monitor, runtimes::Runtime};

/// An Instance ID, unique for each [`Instance`].
pub type InstanceId = u32;

/// An instance, owning a running [`Runtime`].
#[derive(Debug)]
pub struct Instance<RT, S, W> {
    runtime: RT,
    state: Option<S>,
    worker: Option<W>,
    core: CoreId,
}

/// An [`Instance`] representation, used to identify an instance.
#[derive(Debug)]
pub struct InstanceRepr<D> {
    // the PID of the instance
    pid: Pid,
    // the descriptor
    descriptor: D,
}

/// A collection of [`Instance`]s.
///
/// It should contain all the instances being run.
#[derive(Debug)]
pub struct Instances<D, RT, S, W> {
    instances: Vec<Instance<RT, S, W>>,
    active_instances: HashSet<InstanceRepr<D>>,
}

impl<RT, S, W> Instance<RT, S, W>
where
    RT: Runtime<S, W> + 'static,
{
    /// # Safety
    ///
    /// This will spawn a new process, which could have side effects.
    /// Once spawned, the parent process will take back the hand on the control flow immediately.
    pub unsafe fn spawn<CT>(&mut self, controller: &mut CT) -> Result<InstanceRepr<CT::Descriptor>>
    where
        CT: Controller<Worker = W>,
        W: Worker<Controller = CT>,
    {
        // take these out before fork, to mark these as used in the father.
        // the father process will be able to drop the controller in the
        // father process as well.

        let state = self
            .state
            .take()
            .expect("State is not in the instance. This is a fuzzer bug.");

        let mut worker = self
            .worker
            .take()
            .expect("Controller is not in the instance. This is a fuzzer bug.");

        let parent_pid = getpid();

        match unsafe { fork()? } {
            ForkResult::Parent { child } => {
                controller
                    .on_worker_start(worker.descriptor(), child.as_raw().try_into().unwrap())?;

                Ok(InstanceRepr::new(child, worker.descriptor().clone()))
            }
            ForkResult::Child => {
                set_pdeathsig(Signal::SIGKILL)?;

                if getppid() != parent_pid {
                    // race condition between set_pdeathsig call and parent dying.
                    exit(0);
                }

                self.core.set_affinity()?;

                worker.pre_runtime_exec()?;

                // start the child runtime
                self.runtime.run(state, worker)?;

                // TODO: what should we do there in case it happens?
                // i'll panic for now, but it's not the right solution
                panic!("The runtime finished but did not exit cleanly.");
            }
        }
    }
}

impl<D, RT, S, W> Default for Instances<D, RT, S, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D, RT, S, W> Instances<D, RT, S, W> {
    /// Create a new [`Instances`] collection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            active_instances: HashSet::new(),
        }
    }

    /// Add an [`Instance`] to the collection.
    pub fn add_instance(&mut self, instance: Instance<RT, S, W>) {
        self.instances.push(instance);
    }
}

impl<D, RT, S, W> Instances<D, RT, S, W>
where
    W: Worker,
    RT: Runtime<S, W> + 'static,
{
    /// Spawn all [`Instance`]s being owned by [`Self`].
    pub fn spawn_instances<CT>(&mut self, controller: &mut CT) -> Result<()>
    where
        CT: Controller<Worker = W, Descriptor = D>,
        W: Worker<Controller = CT>,
    {
        for instance in &mut self.instances {
            unsafe {
                self.active_instances.insert(instance.spawn(controller)?);
            }
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
        W: Worker<Controller = CT>,
        CT: Controller<Worker = W, Descriptor = D>,
        MT: Monitor,
    {
        let mut sigset = SigSet::empty();
        sigset.add(Signal::SIGCHLD);
        sigprocmask(SigmaskHow::SIG_BLOCK, Some(&sigset), None)
            .map_err(|e| Error::runtime(format!("sigprocmask failed: {e}")))?;

        let sfd = SignalFd::with_flags(&sigset, SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC)
            .map_err(|e| Error::runtime(format!("signalfd failed: {e}")))?;

        // collect children that exited before we set up the signalfd.
        self.drain_children(controller)?;

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
                    self.drain_children(controller)?;
                }
            }
        }

        log::info!("All instances finished successfully.");

        Ok(())
    }

    /// collect dead children correctly.
    fn drain_children<CT>(&mut self, controller: &mut CT) -> Result<()>
    where
        CT: Controller<Worker = W, Descriptor = D>,
        W: Worker<Controller = CT>,
    {
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::ECHILD) => break,
                Ok(WaitStatus::Exited(pid, exit_code)) => {
                    log::info!("Worker with PID {pid} exited with exit code {exit_code}");

                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(|| panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug."));

                    controller.on_worker_exit(&instance_repr.descriptor, exit_code)?;
                }
                Ok(WaitStatus::Signaled(pid, signal, _)) => {
                    log::info!("Worker with PID {pid} exited because of signal {signal}");

                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(|| panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug."));

                    controller.on_worker_termination(&instance_repr.descriptor, signal)?;
                }
                Ok(_) => {}
                Err(e) => return Err(Error::runtime(format!("waitpid failed: {e}"))),
            }
        }
        Ok(())
    }
}

impl<D> InstanceRepr<D> {
    /// Create a new [`Instance`] representant.
    pub fn new(pid: Pid, descriptor: D) -> Self {
        Self { pid, descriptor }
    }
}

impl<D> Borrow<Pid> for InstanceRepr<D> {
    fn borrow(&self) -> &Pid {
        &self.pid
    }
}

impl<D> PartialEq for InstanceRepr<D> {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}

impl<D> Eq for InstanceRepr<D> {}

impl<D> Hash for InstanceRepr<D> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.pid.hash(state);
    }
}

impl<RT, S, W> Instance<RT, S, W> {
    /// Create a new instance.
    pub fn new(runtime: RT, state: S, worker: W, core: CoreId) -> Self {
        Self {
            runtime,
            state: Some(state),
            worker: Some(worker),
            core,
        }
    }
}
