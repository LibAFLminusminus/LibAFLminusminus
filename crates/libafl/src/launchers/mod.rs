use crate::{
    Error, Result, SimpleGlobalController,
    inputs::NopInput,
    monitors::SimpleMonitor,
    runtimes::{RuntimeHandle, StdRuntime, nop::NopRuntime, simple::SimpleRuntime},
    state::NopState,
};
use core::{marker::PhantomData, num::NonZeroUsize};
use libafl_bolts::core_affinity::{CoreId, Cores};
use nix::unistd::Pid;
use serde::Serialize;
use std::vec::Vec;

pub struct StdLauncherBuilder<GCT, MT, RT, S, SB> {
    global_controller: GCT,
    monitor: MT,
    runtime: RT,
    cores: Cores,
    state_builder: SB,
    ram_limit: Option<NonZeroUsize>,
    phantom: PhantomData<S>,
}

pub struct Instance<RT, S> {
    runtime: RT,
    state: S,
    core: CoreId,
    pid: Option<Pid>,
}

pub struct StdLauncher<GCT, MT, RT, S> {
    global_controller: GCT,
    monitor: MT,
    instances: Vec<Instance<RT, S>>,
}

impl<RT, S> Instance<RT, S> {
    pub fn new(runtime: RT, state: S, core: CoreId) -> Self {
        Self {
            runtime,
            state,
            core,
            pid: None,
        }
    }
}

fn nop_state_builder() -> Result<NopState<NopInput>> {
    Ok(NopState::new())
}

impl
    StdLauncher<
        SimpleGlobalController,
        SimpleMonitor,
        NopRuntime,
        fn() -> Result<NopState<NopInput>>,
    >
{
    pub fn builder() -> Result<
        StdLauncherBuilder<
            SimpleGlobalController,
            SimpleMonitor,
            NopRuntime,
            NopState<NopInput>,
            fn() -> Result<NopState<NopInput>>,
        >,
    > {
        let global_controller = SimpleGlobalController::new();
        let monitor = SimpleMonitor::new(&global_controller)?;
        let runtime = NopRuntime;
        let cores = Cores::none();

        Ok(StdLauncherBuilder {
            global_controller,
            monitor,
            runtime,
            cores,
            state_builder: nop_state_builder,
            ram_limit: None,
            phantom: PhantomData,
        })
    }
}

impl<GCT, MT, RT, S> StdLauncher<GCT, MT, RT, S> {
    pub fn new(global_controller: GCT, monitor: MT, instances: Vec<Instance<RT, S>>) -> Self {
        Self {
            global_controller,
            monitor,
            instances,
        }
    }

    pub fn launch(self) -> Result<()> {
        Ok(())
    }
}

impl<GCT, MT, RT, S, SB> StdLauncherBuilder<GCT, MT, RT, S, SB> {
    pub fn runtime<RT2>(self, runtime: RT2) -> StdLauncherBuilder<GCT, MT, RT2, S, SB> {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime,
            state_builder: self.state_builder,
            ram_limit: self.ram_limit,
            phantom: self.phantom,
        }
    }

    pub fn state_builder<S2, SB2>(
        self,
        state_builder: SB2,
    ) -> StdLauncherBuilder<GCT, MT, RT, S2, SB2>
    where
        SB2: FnMut() -> Result<S2>,
    {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: state_builder,
            ram_limit: self.ram_limit,
            phantom: PhantomData::<S2>,
        }
    }
}

impl<GCT, MT, RT, S, SB> StdLauncherBuilder<GCT, MT, RT, S, SB>
where
    S: Serialize,
{
    /// Set the task, and use the standard runtime
    pub fn task<CT, T>(self, task: T) -> StdLauncherBuilder<GCT, MT, StdRuntime<S, T>, S, SB>
    where
        // this bound is needed to help rust link the state output by the state builder and
        // the one used by the task. otherwise, the compiler needs explicit typing.
        T: FnMut(&mut RuntimeHandle<CT, S>, &mut S) -> Result<()>,
    {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: StdRuntime::new(
                task,
                NonZeroUsize::new(1 << 30).expect("RAM limit is invalid"),
            ),
            state_builder: self.state_builder,
            ram_limit: self.ram_limit,
            phantom: self.phantom,
        }
    }
}

impl<GCT, MT, RT, S, SB> StdLauncherBuilder<GCT, MT, RT, S, SB>
where
    RT: Clone,
    SB: FnMut() -> Result<S>,
{
    pub fn build(mut self) -> Result<StdLauncher<GCT, MT, RT, S>> {
        if self.cores.is_empty() {
            return Err(Error::illegal_argument(format!(
                "No cores have been declared."
            )));
        }

        let mut instances: Vec<Instance<RT, S>> = Vec::new();

        // create an instance per core, ready to run.
        for core in self.cores {
            let state: S = (self.state_builder)()?;

            instances.push(Instance::new(self.runtime.clone(), state, core));
        }

        Ok(StdLauncher::new(
            self.global_controller,
            self.monitor,
            instances,
        ))
    }
}
