//! Module defining [`Monitor`]s.

use libafl_bolts::Result;

use crate::Controller;

pub mod simple;
pub use simple::SimpleMonitor;

#[cfg(feature = "web_monitor")]
pub mod web;
#[cfg(feature = "web_monitor")]
pub use web::WebMonitor;

/// This module can show the fuzzer stats to the user via vairous way. like through the terminal or `WebUI` (WIP!)
pub trait Monitor {
    /// Display tick.
    ///
    /// This will be called regularly by the launcher to let the Monitor update its state
    /// and display updated information
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()>;
}
