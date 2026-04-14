//! This module defines trait shared across different `LibAFL` modules

use alloc::boxed::Box;
use core::any::TypeId;
use std::{collections::HashSet, string::String};

#[cfg(feature = "nautilus")]
pub mod nautilus;

use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};

use crate::state::State;

pub struct Registrator {
    map: NamedSerdeAnyMap,
    types: HashSet<TypeId>,
}

pub struct CompatibilityChecker {
    map: NamedSerdeAnyMap,
}

impl Registrator {
    pub fn register_md<T>(&mut self, name: String, value: T) -> Result<(), Error> {
        self.map.add(value)
    }

    pub fn register_md_default<T>(&mut self, name: String) -> Result<(), Error> {
        self.map.add(T::default())
    }

    pub fn register_ty<T: 'static>(&mut self) -> Result<(), Error> {
        self.types.insert(TypeId::of::<T>())
    }

    pub fn finish(self) -> CompatibilityChecker {
        CompatibilityChecker { map: self.map }
    }
}

impl CompatibilityChecker {
    pub fn enforce<T>(&self) -> Result<(), Error> {
        if !self.map.contains::<T>() {
            return Err(Error::not_registered());
        }

        Ok(())
    }

    pub fn enforce_named<T>(&self, name: &str) -> Result<(), Error> {
        if !self.map.contains_named::<T>(name) {
            return Err(Error::not_registered());
        }

        Ok(())
    }

    pub fn finish(self) -> NamedSerdeAnyMap {
        self.map
    }
}

pub trait MetadataResolver: 'static {
    /// Register in the resolver the types necessary during runtime.
    ///
    /// These types will
    fn register(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        Ok(())
    }

    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        registrator.register_ty::<Self>()?;

        self.register(registrator)
    }

    /// Check that some types are actually being used if necessary.
    /// Some objects are interdependent, so we can make sure there is
    fn check(&self, checker: &CompatibilityChecker) -> Result<(), Error> {
        Ok(())
    }
}
