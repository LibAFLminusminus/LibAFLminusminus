//! Dependency resolver for types and metadata.

use core::any;
use std::{collections::HashSet, string::String};

use libafl_bolts::serdeany::{NamedSerdeAnyMap, SerdeAny};
use libafl_core::Result;

use crate::states::add_named_metadata_checked;

/// Dependency registrator.
///
/// Used to register types and metadata used during the fuzzing run.
#[derive(Clone, Debug)]
pub struct Registrator {
    map: NamedSerdeAnyMap,
    types: HashSet<&'static str>,
}

/// Dependency checker.
///
/// Used to check all dependencies are correctly met.
#[derive(Clone, Debug)]
pub struct CompatibilityChecker {
    map: NamedSerdeAnyMap,
    types: HashSet<&'static str>,
}

impl Registrator {
    /// Create a new [`Registrator`]
    pub fn new(state_metadata: NamedSerdeAnyMap) -> Self {
        Self {
            map: state_metadata,
            types: HashSet::new(),
        }
    }

    /// Register a new metadata.
    pub fn register_md<T: SerdeAny>(&mut self, name: String, value: T) {
        add_named_metadata_checked::<T>(&mut self.map, &name, value)
            .expect(format!("Addind same metadata twice: {name}").as_str())
    }

    /// Register a new metadata, with its default value.
    pub fn register_md_default<T: Default + SerdeAny>(&mut self, name: String) {
        add_named_metadata_checked::<T>(&mut self.map, &name, T::default())
            .expect(format!("Addind same metadata twice: {name}").as_str())
    }

    /// Register a new type.
    pub fn register_ty<T: ?Sized>(&mut self) -> bool {
        self.types.insert(any::type_name::<T>())
    }

    /// Finish the registration, and get the [`CompabilityChecker`].
    pub fn finish(self) -> CompatibilityChecker {
        CompatibilityChecker {
            map: self.map,
            types: self.types,
        }
    }
}

impl CompatibilityChecker {
    /// Does the checker contains the given type?
    pub fn contains<T>(&self) -> bool {
        self.types.contains(any::type_name::<T>())
    }

    /// Get the final [`NamedSerdeAnyMap`], produced when all checks are done.
    pub fn finish(self) -> NamedSerdeAnyMap {
        self.map
    }
}

/// Trait for objects with dependencies.
///
/// Every main `LibAFLmm` objects must implement it.
/// That way, it is possible to detect missing types and metadata configuration early.
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
    /// the implementation of [`Self::register_with_ty`] and NOT here. Otherwise, the subtypes
    /// will not be registered correctly.
    fn register(&mut self, _registrator: &mut Registrator) -> Result<()> {
        Ok(())
    }

    /// Register in the resolver the types and metadata necessary during runtime.
    ///
    /// This should be overwritten when registering inner structures.
    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();

        self.register(registrator)
    }

    /// Check that some types (not registered by the current type) are actually being used if necessary.
    /// Some objects are interdependent, so we can make sure one of the objects involved actually registered
    /// the metadata.
    fn check(&self, _checker: &CompatibilityChecker) -> Result<()> {
        Ok(())
    }
}

impl DependencyResolver for () {}

impl<Head, Tail> DependencyResolver for (Head, Tail)
where
    Head: DependencyResolver,
    Tail: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        self.0.register(registrator)?;
        self.1.register(registrator)
    }

    fn register_with_ty(&mut self, registrator: &mut Registrator) -> Result<()> {
        self.0.register_with_ty(registrator)?;
        self.1.register_with_ty(registrator)
    }

    fn check(&self, checker: &CompatibilityChecker) -> Result<()> {
        self.0.check(checker)?;
        self.1.check(checker)
    }
}
