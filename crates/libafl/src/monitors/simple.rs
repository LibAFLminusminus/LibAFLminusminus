use crate::{Controller, Descriptor, Result, Worker, monitors::Monitor, state::read_stats_json};
use core::time::Duration;
use libafl_bolts::current_time;
use quanta::{Clock, Instant};
use std::{fs, path::PathBuf, vec::Vec};

#[derive(Debug, Clone)]
pub struct SimpleMonitor {}

impl SimpleMonitor {
    pub fn new() -> Self {
        SimpleMonitor {}
    }
}

impl Monitor for SimpleMonitor {
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()> {
        for desc in controller.worker_descriptors_mut() {
            if let Some(stats_file) = desc.workdir_mut().get_stats()? {
                let stats = read_stats_json(stats_file)?;
                println!("{}", stats);
            }
        }

        Ok(())
    }
}
