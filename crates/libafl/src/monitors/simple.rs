use crate::{
    Controller, Descriptor, Result,
    monitors::Monitor,
    states::{Stats, read_stats_json},
};
use core::time::Duration;
use libafl_bolts::current_time;
use std::{string::String, vec::Vec};

#[derive(Debug, Clone)]
pub struct SimpleMonitor {}

fn format_si(n: u64) -> String {
    const UNITS: &[(u64, &str)] = &[
        (1_000_000_000_000, "T"),
        (1_000_000_000, "G"),
        (1_000_000, "M"),
        (1_000, "k"),
    ];

    for &(threshold, suffix) in UNITS {
        if n >= threshold {
            return format!("{:.1}{}", n as f64 / threshold as f64, suffix);
        }
    }

    format!("{}", n)
}

fn format_rate(n: u64) -> String {
    format!("{}/s", format_si(n))
}

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

    fn print_summary(&self, all_stats: &[Stats]) {
        let now = current_time();

        let elapsed = all_stats
            .iter()
            .map(|s| s.start_time)
            .min()
            .map(|t| now - t)
            .unwrap_or_default();

        let total_execs: u64 = all_stats.iter().map(|s| s.executions).sum();
        let total_execs_per_sec: u64 = all_stats.iter().map(|s| s.execs_per_sec()).sum();
        let max_corpus: usize = all_stats.iter().map(|s| s.corpus).max().unwrap_or(0);
        let total_objectives: usize = all_stats.iter().map(|s| s.objective).sum();

        let last_find_age = all_stats
            .iter()
            .map(|s| s.last_found_time)
            .max()
            .map(|t| now - t)
            .unwrap_or_default();

        let execs_str = format_si(total_execs);
        let rate_str = format_rate(total_execs_per_sec);

        println!(
            "[{}] {:>3} workers | execs: {:>6} ({:>8}) | corpus: {:>5} | objectives: {:>4} | last find: {} ago",
            format_hhmmss(elapsed),
            all_stats.len(),
            execs_str,
            rate_str,
            max_corpus,
            total_objectives,
            format_hhmmss(last_find_age),
        );
    }
}

impl Monitor for SimpleMonitor {
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()> {
        let mut all_stats: Vec<Stats> = Vec::new();

        for desc in controller.worker_descriptors_mut() {
            if let Some(stats_file) = desc.workdir_mut().get_stats()? {
                all_stats.push(read_stats_json(stats_file)?);
            }
        }

        if !all_stats.is_empty() {
            self.print_summary(&all_stats);
        }

        Ok(())
    }
}
