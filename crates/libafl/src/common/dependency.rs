//! This module defines trait shared across different `LibAFL` modules

use core::any;
use std::{collections::HashSet, string::String};

use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};

use crate::state::add_named_metadata_checked;

pub struct Registrator {
    map: NamedSerdeAnyMap,
    types: HashSet<&'static str>,
}

pub struct CompatibilityChecker {
    map: NamedSerdeAnyMap,
    types: HashSet<&'static str>,
}

impl Registrator {
    pub fn new() -> Self {
        Self {
            map: NamedSerdeAnyMap::new(),
            types: HashSet::new(),
        }
    }

    pub fn register_md<T: SerdeAny>(&mut self, name: String, value: T) -> Result<(), Error> {
        add_named_metadata_checked::<T>(&mut self.map, &name, value)
    }

    pub fn register_md_default<T: Default + SerdeAny>(
        &mut self,
        name: String,
    ) -> Result<(), Error> {
        add_named_metadata_checked::<T>(&mut self.map, &name, T::default())
    }

    pub fn register_ty<T: ?Sized>(&mut self) -> bool {
        self.types.insert(any::type_name::<T>())
    }

    pub fn finish(self) -> CompatibilityChecker {
        CompatibilityChecker {
            map: self.map,
            types: self.types,
        }
    }
}

impl CompatibilityChecker {
    pub fn contains<T>(&self) -> bool {
        self.types.contains(any::type_name::<T>())
    }

    pub fn finish(self) -> NamedSerdeAnyMap {
        self.map
    }
}

pub trait DependencyResolver {
    /// Register in the resolver the metadata necessary during runtime.
    ///
    /// Any global metadata used during runtime MUST be registered there.
    ///
    /// The only exception is Testcase metadata, which can be allocated lazily
    /// at runtime.
    ///
    /// Only register here the metadata.
    /// If you need to propagate this call to inner structucts, ALWAYS do it in
    /// the implementation of `register_with_ty` and NOT here. Otherwise, the subtypes
    /// will not be registered correctly.
    fn register(&mut self, _registrator: &mut Registrator) -> Result<(), Error> {
        Ok(())
    }

    /// Register in the resolver the types and metadata necessary during runtime.
    ///
    /// This should be overwritten when registering inner structures.
    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        registrator.register_ty::<Self>();

        self.register(registrator)
    }

    /// Check that some types (not registered by the current type) are actually being used if necessary.
    /// Some objects are interdependent, so we can make sure one of the objects involved actually registered
    /// the metadata.
    fn check(&self, _checker: &CompatibilityChecker) -> Result<(), Error> {
        Ok(())
    }
}

impl DependencyResolver for () {}

impl<Head, Tail> DependencyResolver for (Head, Tail)
where
    Head: DependencyResolver,
    Tail: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        self.0.register(registrator)?;
        self.1.register(registrator)
    }

    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<(), Error> {
        self.0.register_with_ty(registrator)?;
        self.1.register_with_ty(registrator)
    }

    fn check(&self, checker: &CompatibilityChecker) -> Result<(), Error> {
        self.0.check(checker)?;
        self.1.check(checker)
    }
}
