use crate::{
    Controller, Descriptor, Result, Worker,
    monitors::Monitor,
    states::{Stats, read_stats_json},
};
use core::time::Duration;
use human_repr::{HumanCount, HumanThroughput};
use libafl_bolts::current_time;
use libafl_core::WorkerId;
use quanta::{Clock, Instant};
use std::{fs, path::PathBuf, string::String, vec::Vec};

#[derive(Debug, Clone)]
pub struct SimpleMonitor {}

fn format_hhmmss(duration: Duration) -> String {
    let total_secs = duration.as_secs();

    let hours = total_secs / (60 * 60);
    let mins = (total_secs % (60 * 60)) / 60;
    let secs = total_secs % 60;

    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

impl SimpleMonitor {
    pub fn new() -> Self {
        SimpleMonitor {}
    }

    fn print(&self, worker_id: WorkerId, stats: &Stats) -> Result<()> {
        let time_spent = libafl_bolts::current_time() - stats.start_time;

        let time_spent_formatted = format_hhmmss(time_spent);
        let execs_formatted = stats.executions.human_count_bare();
        let execs_per_sec_formatted = stats.execs_per_sec().human_throughput_bare();

        println!(
            "[{}] Worker {:02} (PID {}) | execs: {} ({}) | corpus : {} | objectives: {}",
            time_spent_formatted,
            worker_id.0,
            stats.pid,
            execs_formatted,
            execs_per_sec_formatted,
            stats.corpus,
            stats.objective,
        );

        Ok(())
    }
}

impl Monitor for SimpleMonitor {
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()> {
        for desc in controller.worker_descriptors_mut() {
            if let Some(stats_file) = desc.workdir_mut().get_stats()? {
                let stats = read_stats_json(stats_file)?;
                self.print(desc.worker_id(), &stats)?;
            }
        }

        Ok(())
    }
}
