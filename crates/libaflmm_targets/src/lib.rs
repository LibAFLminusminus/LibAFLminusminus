//! `libafl_targets` contains runtime code, injected in the target itself during compilation.
#![doc = include_str!("../README.md")]
// Force a newline, otherwise the "feature flags" section doesn't format correctly in the docs
#![doc = "\n"]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
// For `std::simd`
#![cfg_attr(nightly, feature(portable_simd))]

#[macro_use]
extern crate std;

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

pub mod constants {
    include!(concat!(env!("OUT_DIR"), "/constants.rs"));
}

pub mod libfuzzer;

pub mod exports;

#[cfg(feature = "coverage")]
pub mod coverage;

pub mod sancov;

/// runtime related to comparisons
pub mod cmps;

#[cfg(all(windows, feature = "windows_asan"))]
pub mod windows_asan;

pub mod prelude {
    pub use crate::constants::{
        CMP_MAP_SIZE, CMPLOG_MAP_H, CMPLOG_MAP_W, EDGES_MAP_ALLOCATED_SIZE, EDGES_MAP_DEFAULT_SIZE,
    };

    pub use crate::cmps::{
        AFLppCmpLogInstruction, AFLppCmpLogRoutine, AFLppCmplogVals, AFLppLibAFLCmpLogHeader,
        CMPLOG_KIND_INS, CMPLOG_KIND_RTN, CMPLOG_MAP_RTN_H, CMPLOG_MAP_SIZE, CMPLOG_RTN_LEN,
    };

    #[cfg(feature = "coverage")]
    pub use crate::coverage::{MAX_EDGES_FOUND, autotokens, edges_map_mut_ptr};

    #[cfg(all(
        feature = "coverage",
        any(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts")
    ))]
    pub use crate::coverage::edges_map_mut_slice;

    pub use crate::exports::{CMP_MAP, CMPLOG_ENABLED, CMPLOG_MAP};

    #[cfg(feature = "cmplog")]
    pub use crate::exports::CMPLOG_MAP_PTR;

    #[cfg(feature = "coverage")]
    pub use crate::exports::{EDGES_MAP, EDGES_MAP_PTR, INPUT_LENGTH_PTR, INPUT_PTR, SHM_FUZZING};

    pub use crate::libfuzzer::{libfuzzer_initialize, libfuzzer_test_one_input};

    pub use crate::sancov::{libafl_cmp_map, sancov_value_profile};

    #[cfg(all(windows, feature = "windows_asan"))]
    pub use crate::windows_asan::setup_asan_callback;
}
