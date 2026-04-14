//! This module defines trait shared across different `LibAFL` modules

use alloc::boxed::Box;

#[cfg(feature = "nautilus")]
pub mod nautilus;

use libafl_bolts::{
    Error,
    serdeany::{NamedSerdeAnyMap, SerdeAny},
};

use crate::state::State;

pub trait MetadataResolver {
    /// Resolves the metadata dependency
    /// This method is called before the main fuzzing loop starts
    fn resolve<S: State>(&mut self, _state: &mut S) -> Result<(), Error> {
        Ok(())
    }
}
