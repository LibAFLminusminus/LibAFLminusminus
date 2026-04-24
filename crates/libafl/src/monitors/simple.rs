use crate::{Controller, GlobalController, Result, monitors::Monitor, state::read_stats_json};
use core::time::Duration;
use libafl_bolts::current_time;
use std::{fs, path::PathBuf, vec::Vec};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct SimpleMonitor {
    /// the last time monitor was updated,
    last_update: Duration,
    /// the intervals to update monitor
    update_interval: Duration,
}

impl SimpleMonitor {
    pub fn with_duration(update_interval: Duration) -> Result<Self> {
        Ok(Self {
            last_update: current_time(),
            update_interval,
        })
    }

    pub fn new() -> Result<Self> {
        Self::with_duration(DEFAULT_INTERVAL.clone())
    }
}

impl Monitor for SimpleMonitor {
    fn display<GCT: GlobalController>(&mut self, global_controller: &mut GCT) -> Result<()> {
        if current_time() - self.last_update > self.update_interval {
            self.last_update = current_time();
        } else {
            return Ok(());
        }

        for controller in global_controller.controllers() {
            let stat = read_stats_json(controller.descriptor().)?;
            println!("{}", stat);
        }

        Ok(())
    }
}
