//! This module defines trait shared across different `LibAFL` modules

use alloc::boxed::Box;
use core::{any::TypeId, hash::Hash};
use std::{collections::HashSet, string::String};

#[cfg(feature = "nautilus")]
pub mod nautilus;

use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};

use crate::state::{State, add_named_metadata_checked};

pub struct Registrator {
    map: NamedSerdeAnyMap,
    types: HashSet<TypeId>,
}

pub struct CompatibilityChecker {
    map: NamedSerdeAnyMap,
    types: HashSet<TypeId>,
}

impl Registrator {
    pub fn register_md<T: SerdeAny>(&mut self, name: String, value: T) -> Result<(), Error> {
        add_named_metadata_checked::<T>(&mut self.map, &name, value)
    }

    pub fn register_md_default<T: Default + SerdeAny>(
        &mut self,
        name: String,
    ) -> Result<(), Error> {
        add_named_metadata_checked::<T>(&mut self.map, &name, T::default())
    }

    pub fn register_ty<T: 'static>(&mut self) -> bool {
        self.types.insert(TypeId::of::<T>())
    }

    pub fn finish(self) -> CompatibilityChecker {
        CompatibilityChecker {
            map: self.map,
            types: self.types,
        }
    }
}

impl CompatibilityChecker {
    pub fn contains<T: 'static>(&self) -> bool {
        self.types.contains(&TypeId::of::<T>())
    }

    pub fn finish(self) -> NamedSerdeAnyMap {
        self.map
    }
}

pub trait MetadataResolver {
    /// Register in the resolver the types necessary during runtime.
    ///
    /// These types will
    fn register(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        Ok(())
    }

    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        registrator.register_ty::<Self>();

        self.register(registrator)
    }

    /// Check that some types are actually being used if necessary.
    /// Some objects are interdependent, so we can make sure there is
    fn check(&self, checker: &CompatibilityChecker) -> Result<(), Error> {
        Ok(())
    }
}
