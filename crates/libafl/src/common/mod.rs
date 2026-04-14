//! This module defines trait shared across different `LibAFL` modules

use alloc::boxed::Box;

#[cfg(feature = "nautilus")]
pub mod nautilus;

use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};

use crate::state::State;

pub struct Resolver {
    map: NamedSerdeAnyMap,
}

impl Resolver {
    pub fn register<T>(&mut self, name: String) -> Result<(), Error> {
        self.map.add(T::default())
    }

    pub fn finish(self) -> NamedSerdeAnyMap {
        self.map
    }
}

pub trait MetadataResolver {
    /// Resolves the metadata dependency
    /// This method is called before the main fuzzing loop starts
    fn resolve(&mut self, _resolver: &mut Resolver) -> Result<(), Error> {
        Ok(())
    }
}
