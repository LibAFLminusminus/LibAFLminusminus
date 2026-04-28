#[cfg(unix)]
pub mod signal;
use libafl_bolts::{AnonShmBuilder, AnonShmReceiver, AnonShmSender};
#[cfg(unix)]
pub use signal::{OsTerminationHandler, OsTerminationParams};

pub type OsShmSender<S> = AnonShmSender<usize, S>;
pub type OsShmReceiver<S> = AnonShmReceiver<usize, S>;
pub type OsShmBuilder = AnonShmBuilder;
