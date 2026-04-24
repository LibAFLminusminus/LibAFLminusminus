#[cfg(unix)]
pub mod unix;
#[cfg(unix)]
pub use unix::{OsTerminationHandler, OsTerminationParams};

#[cfg(windows)]
pub mod windows;

pub mod termination;
pub use termination::{IntoTerminationHandlerData, TerminationHandler, TerminationHandlerData};
