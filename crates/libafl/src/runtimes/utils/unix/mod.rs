#[cfg(unix)]
pub mod signal;
#[cfg(unix)]
pub use signal::{OsTerminationHandler, OsTerminationParams};

#[cfg(windows)]
pub mod exception;
#[cfg(windows)]
pub use exception::{OsTerminationHandler, OsTerminationParams};
