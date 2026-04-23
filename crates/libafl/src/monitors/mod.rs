use alloc::{string::String, vec::Vec};
use core::{fmt, time::Duration};
use std::{
    fs,
    path::{Path, PathBuf},
    thread::current,
};

use libafl_bolts::{Error, current_time};
use nix::sys::ptrace::interrupt;
use serde::{Deserialize, Serialize};

use crate::{
    Controller, GlobalController,
    fuzzer::HasObjective,
    runtimes::RuntimeHandle,
    state::{FlatState, read_stats_json},
};

pub trait Monitor {
    /// display (or not because you only want to display into the terminal if you are a main instance)
    fn display(&mut self) -> Result<(), Error>;
}

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMonitor {
    /// the path that this instance writes its stats to
    paths: Vec<PathBuf>,
    /// the last time monitor was updated,
    last_update: Duration,
    /// the intervals to update monitor
    update_interval: Duration,
}

impl SimpleMonitor {
    pub fn with_duration<GCT: GlobalController>(
        global_controller: &GCT,
        update_interval: Duration,
    ) -> Result<Self, Error> {
        let paths: Vec<PathBuf> = global_controller
            .controllers()
            .iter()
            .map(|c| c.workdir().join("fuzzer_stats"))
            .collect();
        for p in &paths {
            // remove obsolete stats files.
            if p.exists() && p.is_file() {
                fs::remove_file(p)?;
            }
        }

        Ok(Self {
            paths,
            last_update: current_time(),
            update_interval,
        })
    }

    pub fn new<GCT: GlobalController>(global_controller: &GCT) -> Result<Self, Error> {
        Self::with_duration(global_controller, DEFAULT_INTERVAL.clone())
    }
}

impl Monitor for SimpleMonitor {
    fn display(&mut self) -> Result<(), Error> {
        if current_time() - self.last_update > self.update_interval {
            self.last_update = current_time();
        } else {
            return Ok(());
        }

        for path in &self.paths {
            let stat = read_stats_json(&path)?;
            println!("{}", stat);
        }
        Ok(())
    }
}
