pub mod saver;
pub use saver::OsSaver;

pub mod signal;
pub use signal::{OsTerminationHandler, OsTerminationParams};
