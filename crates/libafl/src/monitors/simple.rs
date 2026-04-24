use crate::{Controller, Result, Worker, monitors::Monitor, state::read_stats_json};
use core::time::Duration;
use libafl_bolts::current_time;
use quanta::{Clock, Instant};
use std::{fs, path::PathBuf, vec::Vec};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct SimpleMonitor {
    clock: Clock,
    /// the last time monitor was updated,
    last_update: Instant,
    /// the intervals to update monitor
    update_interval: Duration,
}

impl SimpleMonitor {
    pub fn with_duration(update_interval: Duration) -> Result<Self> {
        let clock = Clock::new();
        let now = clock.now();

        Ok(Self {
            clock,
            last_update: now,
            update_interval,
        })
    }

    pub fn new() -> Result<Self> {
        Self::with_duration(DEFAULT_INTERVAL.clone())
    }
}

impl Monitor for SimpleMonitor {
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()> {
        let now = self.clock.now();

        if now - self.last_update > self.update_interval {
            self.last_update = now;
        } else {
            return Ok(());
        }

        // TODO: fix print stats
        // for controller in controller.workers() {
        //     let stat = read_stats_json(controller.descriptor())?;
        //     println!("{}", stat);
        // }

        Ok(())
    }
}
