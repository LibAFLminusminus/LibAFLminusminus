//! Module defining [`Monitor`]s.

use crate::controllers::Controller;
use libaflmm_bolts::Result;

pub mod simple;
pub use simple::SimpleMonitor;

pub mod perf_stats;
pub use perf_stats::PerfStats;

#[cfg(feature = "web_monitor")]
pub mod web;
#[cfg(feature = "web_monitor")]
pub use web::WebMonitor;

pub type StdMonitor = SimpleMonitor;

/// This module can show the fuzzer stats to the user in various ways, like through the terminal or `WebUI`.
///
/// Note: the monitor should NOT spawn threads.
/// If threads are needed, they should be started in [`Self::start`] to avoid issues.
pub trait Monitor {
    /// Initialize resources used by the [`Monitor`].
    ///
    /// [`StdLauncher`](crate::launchers::StdLauncher) calls it in the parent process once, after all [`Worker`](crate::controllers::Worker)s have been spawned.
    /// This is guaranteed to be called only in a single process, using the [`Controller`].
    fn start<CT: Controller>(&mut self, _controller: &mut CT) -> Result<()> {
        Ok(())
    }

    /// Display tick.
    ///
    /// This will be called regularly by the launcher to let the Monitor update its state
    /// and display updated information
    ///
    /// Keep in mind the exact time at which it will get called is not fixed.
    /// If you would prefer to update at most every few seconds, the display is responsible for
    /// implementing it by limiting the actual display with some timer.
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()>;
}
