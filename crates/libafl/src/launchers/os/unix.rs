use crate::{Controller, Error, Result, WorkdirFile, Worker, monitors::Monitor, runtimes::Runtime};
use core::{borrow::Borrow, hash::Hash, time::Duration};
use libafl_bolts::core_affinity::CoreId;
use nix::{
    sys::wait::{WaitStatus, wait},
    unistd::{ForkResult, Pid, dup2_stderr, dup2_stdout, fork},
};
use std::{collections::HashSet, fs::File, vec::Vec};

pub type InstanceId = u32;

pub struct Instance<RT, S, W> {
    runtime: RT,
    state: Option<S>,
    worker: Option<W>,
    core: CoreId,
    stdout_file: Option<WorkdirFile>,
    stderr_file: Option<WorkdirFile>,
}

pub struct InstanceRepr<D> {
    pid: Pid,
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

        match unsafe { fork()? } {
            ForkResult::Parent { child } => {
                controller
                    .on_worker_start(worker.descriptor(), child.as_raw().try_into().unwrap())?;

                Ok(InstanceRepr::new(child, worker.descriptor().clone()))
            }
            ForkResult::Child => {
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

    pub fn wait_instances<CT, MT>(&mut self, controller: &mut CT, monitor: &mut MT) -> Result<()>
    where
        W: Worker<Controller = CT>,
        CT: Controller<Worker = W, Descriptor = D>,
        MT: Monitor,
    {
        while !self.active_instances.is_empty() {
            match wait() {
                Ok(WaitStatus::Exited(pid, exit_code)) => {
                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(||
                            panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug.")
                        );

                    controller.on_worker_exit(&instance_repr.descriptor, exit_code)?;
                }
                Ok(WaitStatus::Signaled(pid, signal, _)) => {
                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(||
                            panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug.")
                        );

                    controller.on_worker_termination(&instance_repr.descriptor, signal)?;
                }

                Ok(_) => {
                    // ignore, this is harmless stuff
                }
                Err(e) => return Err(Error::runtime(format!("wait() failed: {e}"))),
            }
        }

        log::info!("All instances finished successfully.");

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
    pub fn new(
        runtime: RT,
        state: S,
        worker: W,
        core: CoreId,
        stdout_file: Option<WorkdirFile>,
        stderr_file: Option<WorkdirFile>,
    ) -> Self {
        Self {
            runtime,
            state: Some(state),
            worker: Some(worker),
            core,
            stdout_file,
            stderr_file,
        }
    }
}
