//! This module defines trait shared across different `LibAFL` modules

pub mod ps;
pub use ps::*;

pub mod dependency;
pub use dependency::{CompatibilityChecker, DependencyResolver, Registrator};

#[cfg(feature = "nautilus")]
pub mod nautilus;
