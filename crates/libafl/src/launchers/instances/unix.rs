use crate::{Controller, Error, Result, WorkdirFile, Worker, monitors::Monitor, runtimes::Runtime};
use core::{borrow::Borrow, hash::Hash, time::Duration};
use libafl_bolts::core_affinity::CoreId;
use nix::{
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::{
        prctl::set_pdeathsig,
        signal::{SigSet, SigmaskHow, Signal, sigprocmask},
        signalfd::{SfdFlags, SignalFd},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::{ForkResult, Pid, dup2_stderr, dup2_stdout, fork, getpid, getppid, pipe},
};
use std::{
    collections::HashSet,
    fs::File,
    os::fd::{AsFd, OwnedFd},
    process::exit,
    thread::sleep,
    time::Instant,
    vec::Vec,
};

pub type InstanceId = u32;

pub struct Instance<RT, S, W> {
    runtime: RT,
    state: Option<S>,
    worker: Option<W>,
    core: CoreId,
}

pub struct InstanceRepr<D> {
    // the PID of the instance
    pid: Pid,
    // the descriptor
    descriptor: D,
}

// for now, this is unix-specific.
// it should be per supported os.
// keep os-specific things there as much as possible.
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

                self.runtime.set_timeout(Duration::from_secs(3));

                // start the child runtime
                self.runtime.run(state, worker)?;

                // TODO: what should we do there in case it happens?
                // i'll panic for now, but it's not the right solution
                panic!("The runtime finished but did not exit cleanly.");
            }
        }
    }
}

impl<D, RT, S, W> Instances<D, RT, S, W> {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            active_instances: HashSet::new(),
        }
    }

    pub fn add_instance(&mut self, instance: Instance<RT, S, W>) {
        self.instances.push(instance);
    }
}

impl<D, RT, S, W> Instances<D, RT, S, W>
where
    W: Worker,
    RT: Runtime<S, W> + 'static,
{
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
                Err(nix::errno::Errno::EINTR) => {
                    // Interrupted by a signal unrelated to SIGCHLD; retry.
                }
                Err(e) => return Err(Error::runtime(format!("poll failed: {e}"))),
                Ok(0) => {
                    // poll timed out. loop over.
                }
                Ok(n) => {
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
                Ok(WaitStatus::StillAlive) => break,
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
                Err(nix::errno::Errno::ECHILD) => break,
                Err(e) => return Err(Error::runtime(format!("waitpid failed: {e}"))),
            }
        }
        Ok(())
    }
}

impl<D> InstanceRepr<D> {
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
    pub fn new(runtime: RT, state: S, worker: W, core: CoreId) -> Self {
        Self {
            runtime,
            state: Some(state),
            worker: Some(worker),
            core,
        }
    }
}
