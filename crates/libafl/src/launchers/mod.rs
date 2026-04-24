use alloc::string::String;
use core::{borrow::Borrow, hash::Hash, marker::PhantomData, num::NonZeroUsize, time::Duration};
use std::{
    collections::HashSet,
    fs::File,
    path::{Path, PathBuf},
    thread::sleep,
    vec::Vec,
};

use libafl_bolts::core_affinity::{CoreId, Cores};
use nix::{
    sys::wait::{WaitStatus, wait},
    unistd::{ForkResult, Pid, dup2_stderr, dup2_stdout, fork},
};
use serde::Serialize;

use crate::{
    Controller, Error, Result, Worker,
    inputs::NopInput,
    monitors::{Monitor, SimpleMonitor},
    nop::{NopDescriptor, NopWorker},
    runtimes::{Runtime, RuntimeHandle, StdRuntime, nop::NopRuntime},
    simple::SimpleController,
    state::NopState,
};

// TODO: use a proper heuristic to choose correct ram size
pub const DEFAULT_MAX_STATE_SIZE_PER_CLIENT: NonZeroUsize = NonZeroUsize::new(1 << 30).unwrap();

pub struct StdLauncherBuilder<CT, MT, RT, S, SB> {
    controller: Option<CT>,
    monitor: Option<MT>,
    runtime: RT,
    cores: Cores,
    state_builder: SB,
    max_state_size_per_client: Option<NonZeroUsize>,
    stdout_file: Option<PathBuf>,
    stderr_file: Option<PathBuf>,
    phantom: PhantomData<S>,
}

pub struct Instance<RT, S, W> {
    runtime: RT,
    state: Option<S>,
    worker: Option<W>,
    core: CoreId,
    stdout_file: Option<File>,
    stderr_file: Option<File>,
}

pub struct InstanceRepr<D> {
    pid: Pid,
    descriptor: D,
}

// for now, this is unix-specific.
// it should be per supported os.
// keep os-specific things there as much as possible.
struct Instances<D, RT, S, W> {
    instances: Vec<Instance<RT, S, W>>,
    active_instances: HashSet<InstanceRepr<D>>,
}

pub struct StdLauncher<D, CT, MT, RT, S, W> {
    controller: CT,
    monitor: MT,
    instances: Instances<D, RT, S, W>,
}

impl<RT, S, W> Instance<RT, S, W> {
    fn new(
        runtime: RT,
        state: S,
        worker: W,
        core: CoreId,
        stdout_file: Option<File>,
        stderr_file: Option<File>,
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

impl
    StdLauncher<
        NopWorker,
        NopDescriptor,
        SimpleController,
        SimpleMonitor,
        NopRuntime,
        NopState<NopInput>,
    >
{
    /// Create a default Launcher.
    /// It is configured with a very minimal configuration.
    /// It will spawn one fuzzing core on core 0 and run the provided task or runtime.
    pub fn builder() -> Result<
        StdLauncherBuilder<
            SimpleController,
            SimpleMonitor,
            NopRuntime,
            NopState<NopInput>,
            fn() -> Result<NopState<NopInput>>,
        >,
    > {
        let runtime = NopRuntime;
        let cores = Cores::one();

        Ok(StdLauncherBuilder {
            controller: None,
            monitor: None,
            runtime,
            cores,
            state_builder: || Ok(NopState::new()),
            max_state_size_per_client: None,
            stdout_file: None,
            stderr_file: None,
            phantom: PhantomData,
        })
    }
}

impl<D, CT, MT, RT, S, W> StdLauncher<D, CT, MT, RT, S, W> {
    pub fn new(controller: CT, monitor: MT, instances: Instances<D, RT, S, W>) -> Self {
        Self {
            controller,
            monitor,
            instances,
        }
    }
}

impl<D, CT, MT, RT, S, W> StdLauncher<D, CT, MT, RT, S, W>
where
    W: Worker<Controller = CT>,
    CT: Controller<Worker = W, Descriptor = D>,
    MT: Monitor,
    RT: Runtime<S, W> + 'static,
{
    pub fn launch(mut self) -> Result<()> {
        self.instances.spawn_instances(&mut self.controller)?;

        self.instances
            .wait_instances(&mut self.controller, &mut self.monitor)?;

        Ok(())
    }
}

impl<CT, MT, RT, S, SB> StdLauncherBuilder<CT, MT, RT, S, SB> {
    pub fn cores(self, cores: Cores) -> StdLauncherBuilder<CT, MT, RT, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        }
    }

    pub fn monitor<MT2>(self, monitor: MT2) -> StdLauncherBuilder<CT, MT2, RT, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: Some(monitor),
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        }
    }

    pub fn controller<CT2>(self, controller: CT2) -> StdLauncherBuilder<CT2, MT, RT, S, SB> {
        StdLauncherBuilder {
            controller: Some(controller),
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        }
    }

    pub fn runtime<RT2>(self, runtime: RT2) -> StdLauncherBuilder<CT, MT, RT2, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        }
    }

    pub fn state_builder<S2, SB2>(
        self,
        state_builder: SB2,
    ) -> StdLauncherBuilder<CT, MT, RT, S2, SB2>
    where
        CT: Controller,
        SB2: FnMut(&CT::Worker) -> Result<S2>,
    {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: PhantomData::<S2>,
        }
    }

    /// Set the RAM limit per client for state.
    ///
    /// Note this is NOT a hard limit: we use it as the memory limit
    /// used to save / restore the state in the restarting runtime.
    ///
    /// The default value is set to [`DEFAULT_MAX_STATE_SIZE_PER_CLIENT`].
    pub fn max_state_size_per_client(
        self,
        max_state_size_per_client: NonZeroUsize,
    ) -> StdLauncherBuilder<CT, MT, RT, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: Some(max_state_size_per_client),
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        }
    }

    /// set the stdout file where the stdout of the fuzzer client should go
    /// point it to dev null if you want to shut it up.
    pub fn stdout_file<P: AsRef<Path>>(
        self,
        stdout_file: &P,
    ) -> StdLauncherBuilder<CT, MT, RT, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: Some(stdout_file.as_ref().to_path_buf()),
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        }
    }

    /// set the stderr file where the stderr of the fuzzer client should go
    /// point it to dev null if you want to shut it up.
    pub fn stderr_file<P: AsRef<Path>>(
        self,
        stderr_file: &P,
    ) -> StdLauncherBuilder<CT, MT, RT, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: Some(stderr_file.as_ref().to_path_buf()),
            phantom: self.phantom,
        }
    }
}

impl<CT, MT, RT, S, SB> StdLauncherBuilder<CT, MT, RT, S, SB>
where
    CT: Controller,
    S: Serialize,
    SB: FnMut(&CT::Worker) -> Result<S>,
{
    pub fn build_with_task<T>(
        mut self,
        task: T,
    ) -> Result<StdLauncher<CT::Descriptor, CT, MT, StdRuntime<S, T>, S, CT::Worker>>
    where
        // this bound is needed to help rust link the state output by the state builder and
        // the one used by the task. otherwise, the compiler needs explicit typing.
        T: FnMut(&mut RuntimeHandle<S, CT::Worker>, &mut S) -> Result<()> + Clone,
    {
        if self.cores.is_empty() {
            return Err(Error::illegal_argument("No cores have been declared."));
        }

        let ram_limit = self
            .max_state_size_per_client
            .unwrap_or(DEFAULT_MAX_STATE_SIZE_PER_CLIENT);

        let builder = StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: StdRuntime::new(task, ram_limit),
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            stdout_file: self.stdout_file,
            stderr_file: self.stderr_file,
            phantom: self.phantom,
        };

        builder.build()
    }
}

impl<CT, MT, RT, S, SB> StdLauncherBuilder<CT, MT, RT, S, SB>
where
    CT: Controller,
    RT: Clone,
    SB: FnMut(&CT::Worker) -> Result<S>,
{
    pub fn build(mut self) -> Result<StdLauncher<CT::Descriptor, CT, MT, RT, S, CT::Worker>> {
        if self.cores.is_empty() {
            return Err(Error::illegal_argument(format!(
                "No CPU cores have been allocated."
            )));
        }

        let monitor = self
            .monitor
            .take()
            .ok_or(Error::illegal_argument("No monitor have been set."))?;

        let mut controller = self.controller.take().ok_or(Error::illegal_argument(
            "No global controller have been set.",
        ))?;

        let mut instances: Instances<CT::Descriptor, RT, S, CT::Worker> = Instances::new();

        let stdout_file: Option<File> = self
            .stdout_file
            .as_ref()
            .map(|p| {
                File::create(p)
                    .map_err(|e| Error::runtime(format!("Failed to open stdout_file: {e}")))
            })
            .transpose()?;

        let stderr_file: Option<File> = self
            .stderr_file
            .as_ref()
            .map(|p| {
                File::create(p)
                    .map_err(|e| Error::runtime(format!("Failed to open stderr_file: {e}")))
            })
            .transpose()?;

        // create an instance per core, ready to run.
        for core in self.cores {
            // spawn a controller for the instance
            let controller = controller.create_controller()?;

            // create the state for the instance
            let state: S = (self.state_builder)(&controller)?;

            // add the instance to the list
            instances.add_instance(Instance::new(
                self.runtime.clone(),
                state,
                controller,
                core,
                match stdout_file.as_ref() {
                    Some(f) => Some(f.try_clone().map_err(|e| {
                        Error::runtime(format!("Failed to clone stdout_file: {e}"))
                    })?),
                    None => None,
                },
                match stderr_file.as_ref() {
                    Some(f) => Some(f.try_clone().map_err(|e| {
                        Error::runtime(format!("Failed to clone stderr_file: {e}"))
                    })?),
                    None => None,
                },
            ));
        }

        Ok(StdLauncher::new(controller, monitor, instances))
    }
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

        let worker = self
            .worker
            .take()
            .expect("Controller is not in the instance. This is a fuzzer bug.");

        match unsafe { fork()? } {
            ForkResult::Parent { child } => {
                controller.on_start(worker.descriptor())?;

                Ok(InstanceRepr::new(child, worker.descriptor().clone()))
            }
            ForkResult::Child => {
                self.core.set_affinity()?;
                if let Some(ref f) = self.stdout_file {
                    dup2_stdout(f)?;
                }
                if let Some(ref f) = self.stderr_file {
                    dup2_stderr(f)?;
                }

                println!("LOL");
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
        // TODO: create a proper even-based loop, i'll do later on.
        sleep(Duration::from_secs(5));
        monitor.display(controller)?;

        while !self.active_instances.is_empty() {
            match wait() {
                Ok(WaitStatus::Exited(pid, exit_code)) => {
                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(||
                            panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug.")
                        );

                    controller.on_exit(&instance_repr.descriptor, exit_code)?;
                }
                Ok(WaitStatus::Signaled(pid, signal, _)) => {
                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(||
                            panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug.")
                        );

                    controller.on_termination(&instance_repr.descriptor, signal)?;
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
