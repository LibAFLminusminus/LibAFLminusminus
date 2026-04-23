use libafl_bolts::core_affinity::CoreId;
use libafl_core::Error;
use nix::unistd::Pid;
use std::vec::Vec;

use crate::{
    SimpleGlobalController,
    monitors::SimpleMonitor,
    runtimes::{nop::NopRuntime, simple::SimpleRuntime},
};

pub struct StdLauncherBuilder<GCT, MT, RT> {
    global_controller: GCT,
    monitor: MT,
    runtime: RT,
}

pub struct Instance<RT> {
    core: CoreId,
    pid: Option<Pid>,
    runtime: RT,
}

pub struct StdLauncher<GCT, MT, RT> {
    global_controller: GCT,
    monitor: MT,
    instances: Vec<Instance<RT>>,
}

impl StdLauncher<SimpleGlobalController, SimpleMonitor, NopRuntime> {
    pub fn builder()
    -> Result<StdLauncherBuilder<SimpleGlobalController, SimpleMonitor, NopRuntime>, Error> {
        let global_controller = SimpleGlobalController::new();
        let monitor = SimpleMonitor::new(&global_controller)?;
        let runtime = NopRuntime;

        Ok(StdLauncherBuilder {
            global_controller,
            monitor,
            runtime: NopRuntime,
        })
    }
}

impl<GCT, MT, RT> StdLauncher<GCT, MT, RT> {
    pub fn new(global_controller: GCT, monitor: MT, instances: Vec<Instance<RT>>) -> Self {
        Self {
            global_controller,
            monitor,
            instances,
        }
    }

    pub fn launch(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<GCT, MT, RT> StdLauncherBuilder<GCT, MT, RT> {
    pub fn runtime<RT2>(self, runtime: RT2) -> StdLauncherBuilder<GCT, MT, RT2> {
        StdLauncherBuilder {
            global_controller: self.global_controller,
            monitor: self.monitor,
            runtime,
        }
    }
}
