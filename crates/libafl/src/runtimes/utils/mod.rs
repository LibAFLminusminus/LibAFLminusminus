pub mod unix;
pub use unix::{OsTerminationHandler, OsTerminationParams};

pub mod termination;
pub use termination::{IntoTerminationHandlerData, TerminationHandler, TerminationHandlerData};
