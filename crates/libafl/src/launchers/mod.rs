use crate::{
    Controller, Error, GlobalController, Result,
    inputs::NopInput,
    monitors::SimpleMonitor,
    nop::{NopController, NopDescriptor, NopGlobalController},
    runtimes::{Runtime, RuntimeHandle, StdRuntime, nop::NopRuntime},
    state::NopState,
};
use core::{borrow::Borrow, hash::Hash, marker::PhantomData, num::NonZeroUsize};
use libafl_bolts::core_affinity::{CoreId, Cores};
use nix::{
    sys::wait::{WaitStatus, wait},
    unistd::{ForkResult, Pid, fork},
};
use serde::Serialize;
use std::{collections::HashSet, vec::Vec};

// TODO: use a proper heuristic to choose correct ram size
pub const DEFAULT_MAX_STATE_SIZE_PER_CLIENT: NonZeroUsize = NonZeroUsize::new(1 << 30).unwrap();

pub struct StdLauncherBuilder<GCT, MT, RT, S, SB> {
    global_controller: GCT,
    monitor: MT,
    runtime: RT,
    cores: Cores,
    state_builder: SB,
    max_state_size_per_client: Option<NonZeroUsize>,
    phantom: PhantomData<S>,
}

pub struct Instance<CT, RT, S> {
    runtime: RT,
    state: Option<S>,
    controller: Option<CT>,
    core: CoreId,
}

pub struct InstanceRepr<D> {
    pid: Pid,
    descriptor: D,
}

// for now, this is unix-specific.
// it should be per supported os.
// keep os-specific things there as much as possible.
pub struct Instances<CT, D, RT, S> {
    instances: Vec<Instance<CT, RT, S>>,
    active_instances: HashSet<InstanceRepr<D>>,
}

pub struct StdLauncher<CT, D, GCT, MT, RT, S> {
    global_controller: GCT,
    monitor: MT,
    instances: Instances<CT, D, RT, S>,
}

impl<CT, RT, S> Instance<CT, RT, S> {
    pub fn new(runtime: RT, state: S, controller: CT, core: CoreId) -> Self {
        Self {
            runtime,
            state: Some(state),
            controller: Some(controller),
            core,
        }
    }
}

impl
    StdLauncher<
        NopController,
        NopDescriptor,
        NopGlobalController,
        SimpleMonitor,
        NopRuntime,
        fn() -> Result<NopState<NopInput>>,
    >
{
    pub fn builder() -> Result<
        StdLauncherBuilder<
            NopGlobalController,
            SimpleMonitor,
            NopRuntime,
            NopState<NopInput>,
            fn() -> Result<NopState<NopInput>>,
        >,
    > {
        let global_controller = NopGlobalController;
        let monitor = SimpleMonitor::new(&global_controller)?;
        let runtime = NopRuntime;
        let cores = Cores::none();

        Ok(StdLauncherBuilder {
            global_controller,
            monitor,
            runtime,
            cores,
            state_builder: || Ok(NopState::new()),
            max_state_size_per_client: None,
            phantom: PhantomData,
        })
    }
}

impl<CT, D, GCT, MT, RT, S> StdLauncher<CT, D, GCT, MT, RT, S> {
    pub fn new(global_controller: GCT, monitor: MT, instances: Instances<CT, D, RT, S>) -> Self {
        Self {
            global_controller,
            monitor,
            instances,
        }
    }
}

impl<CT, D, GCT, MT, RT, S> StdLauncher<CT, D, GCT, MT, RT, S>
where
    GCT: GlobalController<Controller = CT, Descriptor = D>,
    CT: Controller<GlobalController = GCT>,
    RT: Runtime<CT, S> + 'static,
{
    pub fn launch(mut self) -> Result<()> {
        self.instances
            .spawn_instances(&mut self.global_controller)?;

        self.instances.wait_instances(&mut self.global_controller)?;

        Ok(())
    }
}

impl<GCT, MT, RT, S, SB> StdLauncherBuilder<GCT, MT, RT, S, SB> {
    pub fn cores(self, cores: Cores) -> StdLauncherBuilder<GCT, MT, RT, S, SB> {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            phantom: self.phantom,
        }
    }

    pub fn runtime<RT2>(self, runtime: RT2) -> StdLauncherBuilder<GCT, MT, RT2, S, SB> {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            phantom: self.phantom,
        }
    }

    pub fn state_builder<S2, SB2>(
        self,
        state_builder: SB2,
    ) -> StdLauncherBuilder<GCT, MT, RT, S2, SB2>
    where
        GCT: GlobalController,
        SB2: FnMut(&GCT::Controller) -> Result<S2>,
    {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
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
    ) -> StdLauncherBuilder<GCT, MT, RT, S, SB> {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: Some(max_state_size_per_client),
            phantom: self.phantom,
        }
    }
}

impl<GCT, MT, RT, S, SB> StdLauncherBuilder<GCT, MT, RT, S, SB>
where
    GCT: GlobalController,
    S: Serialize,
    SB: FnMut(&GCT::Controller) -> Result<S>,
{
    pub fn build_with_task<T>(
        self,
        task: T,
    ) -> Result<StdLauncher<GCT::Controller, GCT::Descriptor, GCT, MT, StdRuntime<S, T>, S>>
    where
        // this bound is needed to help rust link the state output by the state builder and
        // the one used by the task. otherwise, the compiler needs explicit typing.
        T: FnMut(&mut RuntimeHandle<GCT::Controller, S>, &mut S) -> Result<()> + Clone,
    {
        let ram_limit = self
            .max_state_size_per_client
            .unwrap_or_else(|| DEFAULT_MAX_STATE_SIZE_PER_CLIENT);

        let builder = StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: StdRuntime::new(task, ram_limit),
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            phantom: self.phantom,
        };

        builder.build()
    }
}

impl<GCT, MT, RT, S, SB> StdLauncherBuilder<GCT, MT, RT, S, SB>
where
    GCT: GlobalController,
    RT: Clone,
    SB: FnMut(&GCT::Controller) -> Result<S>,
{
    pub fn build(
        mut self,
    ) -> Result<StdLauncher<GCT::Controller, GCT::Descriptor, GCT, MT, RT, S>> {
        if self.cores.is_empty() {
            return Err(Error::illegal_argument(format!(
                "No cores have been declared."
            )));
        }

        let mut instances: Instances<GCT::Controller, GCT::Descriptor, RT, S> = Instances::new();

        // create an instance per core, ready to run.
        for core in self.cores {
            // spawn a controller for the instance
            let controller = self.global_controller.create_controller()?;

            // create the state for the instance
            let state: S = (self.state_builder)(&controller)?;

            // add the instance to the list
            instances.add_instance(Instance::new(self.runtime.clone(), state, controller, core));
        }

        Ok(StdLauncher::new(
            self.global_controller,
            self.monitor,
            instances,
        ))
    }
}

impl<CT, RT, S> Instance<CT, RT, S>
where
    RT: Runtime<CT, S> + 'static,
{
    /// # Safety
    ///
    /// This will spawn a new process, which could have side effects.
    /// Once spawned, the parent process will take back the hand on the control flow immediately.
    pub unsafe fn spawn<GCT>(
        &mut self,
        global_controller: &mut GCT,
    ) -> Result<InstanceRepr<GCT::Descriptor>>
    where
        GCT: GlobalController<Controller = CT>,
        CT: Controller<GlobalController = GCT>,
    {
        // take these out before fork, to mark these as used in the father.
        // the father process will be able to drop the controller in the
        // father process as well.

        let state = self
            .state
            .take()
            .expect("State is not in the instance. This is a fuzzer bug.");

        let controller = self
            .controller
            .take()
            .expect("Controller is not in the instance. This is a fuzzer bug.");

        match unsafe { fork()? } {
            ForkResult::Parent { child } => {
                global_controller.on_start(controller.descriptor())?;

                Ok(InstanceRepr::new(child, controller.descriptor().clone()))
            }
            ForkResult::Child => {
                self.core.set_affinity()?;

                // start the child runtime
                self.runtime.run(state, controller)?;

                // TODO: what should we do there in case it happens?
                // i'll panic for now, but it's not the right solution
                panic!("The runtime finished but did not exit cleanly.");
            }
        }
    }
}

impl<CT, D, RT, S> Instances<CT, D, RT, S> {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            active_instances: HashSet::new(),
        }
    }

    pub fn add_instance(&mut self, instance: Instance<CT, RT, S>) {
        self.instances.push(instance);
    }
}

impl<CT, D, RT, S> Instances<CT, D, RT, S>
where
    CT: Controller,
    RT: Runtime<CT, S> + 'static,
{
    pub fn spawn_instances<GCT>(&mut self, global_controller: &mut GCT) -> Result<()>
    where
        GCT: GlobalController<Controller = CT, Descriptor = D>,
        CT: Controller<GlobalController = GCT>,
    {
        for instance in &mut self.instances {
            unsafe {
                self.active_instances
                    .insert(instance.spawn(global_controller)?);
            }
        }

        Ok(())
    }

    pub fn wait_instances<GCT>(&mut self, global_controller: &mut GCT) -> Result<()>
    where
        GCT: GlobalController<Controller = CT, Descriptor = D>,
        CT: Controller<GlobalController = GCT>,
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

                    global_controller.on_exit(&instance_repr.descriptor, exit_code)?;
                }
                Ok(WaitStatus::Signaled(pid, signal, _)) => {
                    let instance_repr = self
                        .active_instances
                        .take(&pid)
                        .unwrap_or_else(||
                            panic!("Removed a PID ({pid}) not in the active PID list. This is a fuzzer bug.")
                        );

                    global_controller.on_termination(&instance_repr.descriptor, signal)?;
                }

                Ok(_) => {
                    // ignore, this is harmless stuff
                }
                Err(e) => {
                    panic!("Error while waiting: {e}");
                }
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
