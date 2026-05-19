//! Instances collection.
//!
//! Each supported OS has its own [`Instance`] submodule.

#[cfg(unix)]
pub mod unix;
#[cfg(unix)]
pub use unix::{Instance, InstanceId, InstanceRepr, Instances};
