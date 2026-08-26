//! Module defining launchers.
//! Launchers start the fuzzing session, involving multiple instances.

use crate::{
    Error, Result,
    controllers::{Controller, StdController, StdWorker, Worker, WorkerId},
    launchers::groups::{PendingGroup, RegisteredGroupTuple},
    monitors::{Monitor, SimpleMonitor, StdMonitor},
    runtimes::{NopRuntime, RuntimeHandle},
    states::{NopState, State},
    sync::GroupId,
};
use core::fmt::Debug;
use core::time::Duration;
use libaflmm_bolts::{Cores, StdTimer, timers::Timer};
use libaflmm_core::illegal_argument;
use serde::{Serialize, de::DeserializeOwned};

pub mod instances;
pub use instances::{Instance, InstanceId, Instances};

pub mod groups;
pub use groups::{GenericGroupBuilder, Group, GroupTuple, StdGroup, WorkerLayout};

/// The default time between each monitor refresh.
pub const DEFAULT_MONITOR_REFRESH: Duration = Duration::from_secs(5);

/// The standard group builder, made of a single group.
pub type StdGroupBuilder<I> = GenericGroupBuilder<
    fn(GroupId, WorkerId) -> Result<WorkerLayout>,
    NopRuntime,
    NopState,
    fn(&StdWorker<I>) -> Result<NopState>,
    StdTimer,
    StdWorker<I>,
>;

/// The builder for [`StdLauncher`]
#[derive(Debug)]
pub struct StdLauncherBuilder<CT, GT, MT> {
    controller: Option<CT>,
    monitor: Option<MT>,
    monitor_refresh: Duration,
    groups: GT,
}

/// A standard launcher.
#[derive(Debug)]
pub struct StdLauncher<D, CT, MT, W> {
    controller: CT,
    monitor: MT,
    instances: Instances<D, W>,
    monitor_refresh: Duration,
}

impl<D, CT, MT, W> StdLauncher<D, CT, MT, W> {
    /// Create a new [`StdLauncher`].
    pub fn new(
        controller: CT,
        monitor: MT,
        instances: Instances<D, W>,
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

impl StdLauncher<(), (), (), ()> {
    /// Create a [`StdLauncherBuilder`], for single group and no corpus sharing setup.
    /// Each fuzzer will run independently, with synchronization between them.
    ///
    /// This is the simplest multi-core setup, which provides good results in most cases.
    ///
    /// A [`StdController`] is built on launch if none is set with
    /// [`StdLauncherBuilder::controller`].
    #[must_use]
    pub fn builder<I>() -> StdLauncherBuilder<StdController<I>, StdGroupBuilder<I>, StdMonitor>
    where
        I: Debug,
    {
        StdLauncherBuilder {
            controller: None,
            monitor: Some(StdMonitor::new()),
            monitor_refresh: DEFAULT_MONITOR_REFRESH,
            groups: StdGroup::builder_for::<StdWorker<I>>(),
        }
    }

    /// Get an empty [`StdLauncherBuilder`], used to create more complex multi-group setups.
    ///
    /// The [`Controller`], [`Monitor`] and every [`Group`]
    /// must be set explicitly before [`StdLauncherBuilder::build`].
    #[must_use]
    pub fn empty() -> StdLauncherBuilder<(), (), SimpleMonitor> {
        StdLauncherBuilder {
            controller: None,
            monitor: None,
            monitor_refresh: DEFAULT_MONITOR_REFRESH,
            groups: (),
        }
    }
}

impl<D, CT, MT, W> StdLauncher<D, CT, MT, W>
where
    W: Worker<Descriptor = D>,
    CT: Controller<Worker = W>,
    MT: Monitor,
{
    /// Launch the launcher [`Instance`]s.
    pub fn launch(mut self) -> Result<()> {
        (self.instances, self.controller, self.monitor) = self
            .instances
            .spawn_instances(self.controller, self.monitor)?;

        self.monitor.start(&mut self.controller)?;

        self.instances.wait_instances(
            &mut self.controller,
            &mut self.monitor,
            self.monitor_refresh,
        )?;

        Ok(())
    }
}

impl<CT, GT, MT> StdLauncherBuilder<CT, GT, MT> {
    /// Set the [`Monitor`].
    pub fn monitor<MT2>(self, monitor: MT2) -> StdLauncherBuilder<CT, GT, MT2> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: Some(monitor),
            monitor_refresh: self.monitor_refresh,
            groups: self.groups,
        }
    }

    /// Set the [`Controller`].
    pub fn controller<CT2>(self, controller: CT2) -> StdLauncherBuilder<CT2, GT, MT> {
        StdLauncherBuilder {
            controller: Some(controller),
            monitor: self.monitor,
            monitor_refresh: self.monitor_refresh,
            groups: self.groups,
        }
    }

    /// Set the monitor refresh rate.
    #[must_use]
    pub fn monitor_refresh(self, monitor_refresh: Duration) -> StdLauncherBuilder<CT, GT, MT> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            monitor_refresh,
            groups: self.groups,
        }
    }
}

impl<CT, GT, MT> StdLauncherBuilder<CT, GT, MT>
where
    CT: Controller,
    CT::GroupConfig: Default,
{
    #[expect(clippy::type_complexity)]
    pub fn add_group<G>(
        self,
        group: G,
    ) -> StdLauncherBuilder<CT, (PendingGroup<CT::GroupConfig, G>, GT), MT> {
        self.add_group_with(group, CT::GroupConfig::default())
    }
}

impl<CT, GT, MT> StdLauncherBuilder<CT, GT, MT>
where
    CT: Controller,
{
    #[expect(clippy::type_complexity)]
    pub fn add_group_with<G>(
        self,
        group: G,
        config: CT::GroupConfig,
    ) -> StdLauncherBuilder<CT, (PendingGroup<CT::GroupConfig, G>, GT), MT> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            monitor_refresh: self.monitor_refresh,
            groups: (PendingGroup::new(group, config), self.groups),
        }
    }
}

impl<CT, GT, MT> StdLauncherBuilder<CT, GT, MT>
where
    CT: Controller,
    GT: GroupTuple<CT>,
{
    /// Build the [`StdLauncher`].
    #[expect(clippy::type_complexity)]
    pub fn build(
        mut self,
    ) -> Result<StdLauncher<<CT::Worker as Worker>::Descriptor, CT, MT, CT::Worker>> {
        let monitor = self
            .monitor
            .take()
            .ok_or(Error::illegal_argument("No monitor have been set."))?;

        let mut controller = self.controller.take().ok_or(Error::illegal_argument(
            "No global controller have been set.",
        ))?;

        let mut instances = Instances::new();
        let configured = self.groups.register_all(&mut controller)?;
        controller.finalize_orchestration()?;
        configured.instantiate_all(&mut controller, &mut instances)?;

        if instances.is_empty() {
            return Err(illegal_argument!(
                "No instances have been created. Are groups correctly configured?"
            ));
        }

        Ok(StdLauncher::new(
            controller,
            monitor,
            instances,
            self.monitor_refresh,
        ))
    }
}

// this is for the standard single group builder
impl<CT, L, MT, RT, S, SB, TM, W>
    StdLauncherBuilder<CT, GenericGroupBuilder<L, RT, S, SB, TM, W>, MT>
{
    /// Set the cores associated to each [`Instance`].
    #[must_use]
    pub fn cores(mut self, cores: Cores) -> Self {
        self.groups = self.groups.cores(cores);
        self
    }

    /// Set the timeout of the instance
    #[must_use]
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.groups = self.groups.timeout(timeout);
        self
    }

    /// Set the worker layout closure.
    pub fn worker_layout_fn<L2>(
        self,
        layout_fn: L2,
    ) -> StdLauncherBuilder<CT, GenericGroupBuilder<L2, RT, S, SB, TM, W>, MT> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            monitor_refresh: self.monitor_refresh,
            groups: self.groups.worker_layout_fn(layout_fn),
        }
    }

    /// Set the timer used by the runtime built with `run_inprocess`.
    pub fn timer<TM2>(
        self,
        timer: TM2,
    ) -> StdLauncherBuilder<CT, GenericGroupBuilder<L, RT, S, SB, TM2, W>, MT> {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            monitor_refresh: self.monitor_refresh,
            groups: self.groups.timer(timer),
        }
    }

    /// Set the [`State`] builder closure.
    pub fn state_builder<S2, SB2>(
        self,
        state_builder: SB2,
    ) -> StdLauncherBuilder<CT, GenericGroupBuilder<L, RT, S2, SB2, TM, W>, MT>
    where
        SB2: FnMut(&W) -> Result<S2>,
    {
        StdLauncherBuilder {
            controller: self.controller,
            monitor: self.monitor,
            monitor_refresh: self.monitor_refresh,
            groups: self.groups.state_builder(state_builder),
        }
    }
}

// this is for the standard single group builder
impl<I, L, MT, S, SB, TM>
    StdLauncherBuilder<
        StdController<I>,
        GenericGroupBuilder<L, NopRuntime, S, SB, TM, StdWorker<I>>,
        MT,
    >
where
    I: Debug,
    L: FnMut(GroupId, WorkerId) -> Result<WorkerLayout>,
    MT: Monitor,
    S: State<Input = I> + Serialize + 'static,
    SB: FnMut(&StdWorker<I>) -> Result<S>,
{
    /// Run the single group with an in-process runtime, using the [`Controller`] that has
    /// been set or a default [`StdController`].
    pub fn launch_inprocess<T>(self, task: T) -> Result<()>
    where
        S: DeserializeOwned,
        T: FnMut(&mut RuntimeHandle<S, StdWorker<I>>, &mut S) -> Result<()> + Clone + 'static,
        TM: Timer + Clone + 'static,
    {
        let monitor = self
            .monitor
            .expect("monitor not set, this is a libaflmm bug");
        let monitor_refresh = self.monitor_refresh;
        let controller = match self.controller {
            Some(controller) => controller,
            None => StdController::builder().build()?,
        };
        let group = self.groups.build_inprocess(task)?;

        StdLauncher::empty()
            .controller(controller)
            .monitor(monitor)
            .monitor_refresh(monitor_refresh)
            .add_group(group)
            .build()?
            .launch()
    }

    /// Run the single group with a forkserver runtime, using the [`Controller`] that has
    /// been set or a default [`StdController`].
    pub fn launch_forkserver<T>(self, task: T) -> Result<()>
    where
        T: FnMut(&mut RuntimeHandle<S, StdWorker<I>>, &mut S) -> Result<()> + Clone + 'static,
    {
        let monitor = self.monitor.expect("simple() always sets a monitor");
        let monitor_refresh = self.monitor_refresh;
        let controller = match self.controller {
            Some(controller) => controller,
            None => StdController::builder().build()?,
        };
        let group = self.groups.build_forkserver(task)?;

        StdLauncher::empty()
            .controller(controller)
            .monitor(monitor)
            .monitor_refresh(monitor_refresh)
            .add_group(group)
            .build()?
            .launch()
    }
}
