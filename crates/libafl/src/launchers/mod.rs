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
    Controller, Error, Result, WorkdirFile, Worker,
    inputs::NopInput,
    monitors::{Monitor, SimpleMonitor},
    nop::{NopDescriptor, NopWorker},
    runtimes::{Runtime, RuntimeHandle, StdRuntime, nop::NopRuntime},
    simple::SimpleController,
    state::NopState,
};

pub mod instances;
pub use instances::{Instance, InstanceId, InstanceRepr, Instances};

// TODO: use a proper heuristic to choose correct ram size
pub const DEFAULT_MAX_STATE_SIZE_PER_CLIENT: NonZeroUsize = NonZeroUsize::new(1 << 30).unwrap();
pub const DEFAULT_MONITOR_REFRESH: Duration = Duration::from_secs(2);

pub struct StdLauncherBuilder<CT, MT, RT, S, SB> {
    controller: Option<CT>,
    monitor: Option<MT>,
    runtime: RT,
    cores: Cores,
    state_builder: SB,
    max_state_size_per_client: Option<NonZeroUsize>,
    monitor_refresh: Duration,
    phantom: PhantomData<S>,
}

pub struct StdLauncher<D, CT, MT, RT, S, W> {
    controller: CT,
    monitor: MT,
    instances: Instances<D, RT, S, W>,
    monitor_refresh: Duration,
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
            monitor_refresh: DEFAULT_MONITOR_REFRESH.clone(),
            phantom: PhantomData,
        })
    }
}

impl<D, CT, MT, RT, S, W> StdLauncher<D, CT, MT, RT, S, W> {
    pub fn new(
        controller: CT,
        monitor: MT,
        instances: Instances<D, RT, S, W>,
        monitor_refresh: Duration,
    ) -> Self {
        Self {
            controller,
            monitor,
            instances,
            monitor_refresh,
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

        self.instances.wait_instances(
            &mut self.controller,
            &mut self.monitor,
            self.monitor_refresh.clone(),
        )?;

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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
            phantom: self.phantom,
        }
    }

    /// Set the monitor refresh rate.
    pub fn monitor_refresh(
        self,
        monitor_refresh: Duration,
    ) -> StdLauncherBuilder<CT, MT, RT, S, SB> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            monitor_refresh: monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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
            monitor_refresh: self.monitor_refresh,
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

        // create an instance per core, ready to run.
        for core in self.cores {
            // spawn a controller for the instance
            let worker = controller.create_worker()?;

            // create the state for the instance
            let state: S = (self.state_builder)(&worker)?;

            // add the instance to the list
            instances.add_instance(Instance::new(self.runtime.clone(), state, worker, core));
        }

        Ok(StdLauncher::new(
            controller,
            monitor,
            instances,
            self.monitor_refresh,
        ))
    }
}
