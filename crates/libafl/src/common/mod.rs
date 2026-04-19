//! This module defines trait shared across different `LibAFL` modules

use core::any;
use std::{collections::HashSet, string::String};

use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};

pub mod dependency;
pub use dependency::{CompatibilityChecker, DependencyResolver, Registrator};

#[cfg(feature = "nautilus")]
pub mod nautilus;
