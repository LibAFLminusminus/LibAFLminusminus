//! Unix-specific [`Runtime`] primitives.

use libafl_bolts::{AnonShmBuilder, AnonShmReceiver, AnonShmSender};

#[cfg(unix)]
pub mod signal;
#[cfg(unix)]
pub use signal::{OsTerminationHandler, OsTerminationParams};

/// Os-specific shared memory sender.
pub type OsShmSender<S> = AnonShmSender<usize, S>;

/// Os-specific shared memory receiver.
pub type OsShmReceiver<S> = AnonShmReceiver<usize, S>;

/// Os-specific shared memory builder, building a pair of [`OsShmSender`] / [`OsShmReceiver`].
pub type OsShmBuilder = AnonShmBuilder;
