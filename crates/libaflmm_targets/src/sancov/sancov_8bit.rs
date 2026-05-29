//! [`LLVM` `8-bit-counters`](https://clang.llvm.org/docs/SanitizerCoverage.html#tracing-pcs-with-guards) runtime for `LibAFL`.
use alloc::vec::Vec;
use libaflmm_bolts::OwnedMutSlice;
use libaflmm_core::{AsSlice, AsSliceMut};

/// A [`Vec`] of `8-bit-counters` maps for multiple modules.
/// They are initialized by calling [`__sanitizer_cov_8bit_counters_init`](
pub static mut COUNTERS_MAPS: Vec<OwnedMutSlice<'static, u8>> = Vec::new();

/// Gets a pointer to [`COUNTERS_MAPS`]
///
/// # Safety
/// The resulting pointer points to a global. Handle with care!
#[must_use]
pub unsafe fn counters_maps_ptr() -> *const Vec<OwnedMutSlice<'static, u8>> {
    &raw const COUNTERS_MAPS
}

/// Gets a pointer to [`COUNTERS_MAPS`], mut
///
/// # Safety
/// The resulting pointer points to a global. Handle with care!
pub unsafe fn counters_maps_ptr_mut() -> *mut Vec<OwnedMutSlice<'static, u8>> {
    &raw mut COUNTERS_MAPS
}

/// Create more copies of the counters maps
///
/// # Safety
/// You are responsible for ensuring there is no multi-mutability!
#[must_use]
pub unsafe fn extra_counters() -> Vec<OwnedMutSlice<'static, u8>> {
    let counters_maps = unsafe { &*counters_maps_ptr() };
    counters_maps
        .iter()
        .map(|counters| unsafe {
            OwnedMutSlice::from_raw_parts_mut(
                counters.as_slice().as_ptr().cast_mut(),
                counters.as_slice().len(),
            )
        })
        .collect()
}

/// Initialize the sancov `8-bit-counters` - usually called by `llvm`.
///
/// # Safety
/// Start and stop are being dereferenced.
#[unsafe(no_mangle)]
#[expect(clippy::cast_sign_loss)]
pub unsafe extern "C" fn __sanitizer_cov_8bit_counters_init(start: *mut u8, stop: *mut u8) {
    unsafe {
        let counters_maps = &mut *counters_maps_ptr_mut();
        for existing in counters_maps {
            let range = existing.as_slice_mut().as_mut_ptr()
                ..=existing
                    .as_slice_mut()
                    .as_mut_ptr()
                    .add(existing.as_slice().len());
            if range.contains(&start) || range.contains(&stop) {
                // we have overlapping or touching ranges; merge them
                let &start = range.start().min(&start);
                let &stop = range.end().max(&stop);
                *existing =
                    OwnedMutSlice::from_raw_parts_mut(start, stop.offset_from(start) as usize);
                return;
            }
        }

        let counters_maps = &mut *counters_maps_ptr_mut();
        // we didn't overlap; keep going
        counters_maps.push(OwnedMutSlice::from_raw_parts_mut(
            start,
            stop.offset_from(start) as usize,
        ));
    }
}
