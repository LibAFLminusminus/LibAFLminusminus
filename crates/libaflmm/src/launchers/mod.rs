//! Module defining launchers.
//! Launchers start the fuzzing session, involving multiple instances.

use crate::{
    Error, Result,
    controllers::{Controller, Worker},
    launchers::groups::{GroupTuple, PendingGroup, RegisteredGroupTuple},
    monitors::{Monitor, SimpleMonitor},
};
use core::time::Duration;
use libaflmm_core::illegal_argument;

pub mod instances;
pub use instances::{Instance, InstanceId, Instances};

pub mod groups;
pub use groups::{StdGroup, StdGroupBuilder};

/// The default time between each monitor refresh.
pub const DEFAULT_MONITOR_REFRESH: Duration = Duration::from_secs(5);

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

impl StdLauncher<(), (), (), ()> {
    /// Create a default Launcher.
    /// It is configured with a very minimal configuration.
    /// It will spawn one fuzzing core on core 0 and run the provided task or runtime.
    #[must_use]
    pub fn builder() -> StdLauncherBuilder<(), (), SimpleMonitor> {
        StdLauncherBuilder {
            controller: None,
            monitor: None,
            monitor_refresh: DEFAULT_MONITOR_REFRESH,
            groups: (),
        }
    }
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
