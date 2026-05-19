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

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

pub mod libfuzzer;
pub use libfuzzer::*;

pub mod exports;
pub use exports::*;

#[cfg(feature = "coverage")]
pub mod coverage;
#[cfg(feature = "coverage")]
pub use coverage::*;

pub mod sancov;
pub use sancov::*;

/// runtime related to comparisons
pub mod cmps;
pub use cmps::*;

#[cfg(all(windows, feature = "windows_asan"))]
pub mod windows_asan;
#[cfg(all(windows, feature = "windows_asan"))]
pub use windows_asan::*;
