//! The most simple [`Monitor`]. It gathers data from children instances and dumps the data to stdout

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use libaflmm_bolts::current_time;

use crate::{
    Result, controllers::Controller, controllers::Descriptor, monitors::Monitor, states::Stats,
};

#[derive(Debug, Clone)]
/// The most simple [`Monitor`] for dumping the stats to stdout.
pub struct SimpleMonitor {}

#[expect(clippy::cast_precision_loss)]
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

    format!("{n}")
}

fn format_rate(n: u64) -> String {
    format!("{}/s", format_si(n))
}

fn format_hhmmss(duration: Duration) -> String {
    let total_secs = duration.as_secs();

    let hours = total_secs / (60 * 60);
    let mins = (total_secs % (60 * 60)) / 60;
    let secs = total_secs % 60;

    format!("{hours:02}:{mins:02}:{secs:02}")
}

impl Default for SimpleMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleMonitor {
    /// Construct a [`struct@SimpleMonitor`]
    #[must_use]
    pub fn new() -> Self {
        SimpleMonitor {}
    }

    fn print_summary(all_stats: &[Stats]) {
        let now = current_time();

        let elapsed = all_stats
            .iter()
            .map(|s| s.start_time)
            .min()
            .map(|t| now.checked_sub(t).unwrap())
            .unwrap_or_default();

        let total_execs: u64 = all_stats.iter().map(|s| s.executions).sum();
        let total_execs_per_sec: u64 = all_stats.iter().map(Stats::execs_per_sec).sum();
        let max_corpus: usize = all_stats.iter().map(|s| s.corpus).max().unwrap_or(0);
        let total_objectives: usize = all_stats.iter().map(|s| s.objective).sum();

        let last_find_age = all_stats
            .iter()
            .map(|s| s.last_found_time)
            .max()
            .map(|t| now.checked_sub(t).unwrap())
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
            if let Some(stats) = desc.workdir().read_stats()? {
                all_stats.push(stats);
            }
        }

        if !all_stats.is_empty() {
            Self::print_summary(&all_stats);
        }

        Ok(())
    }
}
