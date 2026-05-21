//! Callbacks for all the sancov weakly defined functions

#[cfg(feature = "sancov_8bit")]
pub mod sancov_8bit;
#[cfg(feature = "sancov_8bit")]
pub use sancov_8bit::*;

#[cfg(any(feature = "sancov_cmplog", feature = "sancov_value_profile"))]
pub mod sancov_cmp;
#[cfg(any(feature = "sancov_cmplog", feature = "sancov_value_profile"))]
pub use sancov_cmp::*;

#[cfg(any(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts",))]
pub mod sancov_pcguard;
#[cfg(any(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts",))]
pub use sancov_pcguard::*;

pub mod sancov_value_profile;
pub use sancov_value_profile::*;
