//! This module defines trait shared across different `LibAFL` modules

use core::any;
use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};
use std::{collections::HashSet, string::String};

pub mod dependency;
pub use dependency::{CompatibilityChecker, DependencyResolver, Registrator};

#[cfg(feature = "nautilus")]
pub mod nautilus;
