//! [`LLVM` `PcGuard`](https://clang.llvm.org/docs/SanitizerCoverage.html#tracing-pcs-with-guards) runtime for `LibAFL`.

use core::slice;

#[cfg(feature = "coverage")]
use crate::coverage::EDGES_MAP;
#[cfg(feature = "coverage")]
use crate::coverage::MAX_EDGES_FOUND;
#[cfg(feature = "pointer_maps")]
use crate::{EDGES_MAP_ALLOCATED_SIZE, coverage::EDGES_MAP_PTR};

#[cfg(all(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts"))]
#[cfg(not(any(doc, feature = "clippy", test)))]
compile_error!(
    "the libafl_targets `sancov_pcguard_edges` and `sancov_pcguard_hitcounts` features are mutually exclusive."
);

static mut PC_TABLES: Vec<&'static [PcTableEntry]> = Vec::new();

/// Type for the PC guard hook
pub type PcGuardHook = unsafe extern "C" fn(*mut u32);

use alloc::vec::Vec;
unsafe extern "C" {
    /// The ctx variable
    pub static mut __afl_prev_ctx: u32;
}

#[allow(clippy::inline_always)]
#[inline(always)]
#[allow(unused_assignments)]
pub(crate) unsafe fn sanitizer_cov_pcguard_impl(guard: *mut u32) {
    unsafe {
        #[allow(unused_variables, unused_mut)] // cfg dependent
        let mut pos = *guard as usize;

        #[cfg(feature = "pointer_maps")]
        {
            #[cfg(feature = "sancov_pcguard_edges")]
            {
                EDGES_MAP_PTR.add(pos).write(1);
            }
            #[cfg(feature = "sancov_pcguard_hitcounts")]
            {
                let addr = EDGES_MAP_PTR.add(pos);
                let val = addr.read().wrapping_add(1);
                addr.write(val);
            }
        }
        #[cfg(not(feature = "pointer_maps"))]
        #[cfg(any(feature = "sancov_pcguard_hitcounts", feature = "sancov_pcguard_edges"))]
        {
            #[cfg(feature = "sancov_pcguard_edges")]
            {
                let p = (core::ptr::addr_of_mut!(EDGES_MAP) as *mut u8).add(pos);
                *p = 1;
            }
            #[cfg(feature = "sancov_pcguard_hitcounts")]
            {
                let p = (core::ptr::addr_of_mut!(EDGES_MAP) as *mut u8).add(pos);
                let val = (*p).wrapping_add(1);
                *p = val;
            }
        }
    }
}

/// Initialize the sancov `pc_guard` - usually called by `llvm`.
///
/// # Safety
/// Dereferences the edges map at `start` and writes to it.
/// Should usually not be called directly, but is called by `llvm`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __sanitizer_cov_trace_pc_guard_init(
    #[allow(unused_mut)] // only mut with the `coverage` feature
    #[allow(unused_variables)] // only used with the `coverage` feature
    mut start: *mut u32,
    #[allow(unused_variables)] // only used with the `coverage` feature
    stop: *mut u32,
) {
    // # Safety
    // Dereferences at `start` and writes to it, as it sais on this function's title.
    // As unsafe as the caller wants it to be.
    #[cfg(feature = "pointer_maps")]
    unsafe {
        if EDGES_MAP_PTR.is_null() {
            EDGES_MAP_PTR = &raw mut EDGES_MAP as *mut u8;
        }
    }

    #[cfg(feature = "coverage")]
    if core::ptr::eq(start, stop) || unsafe { *start != 0 } {
        return;
    }

    #[cfg(feature = "coverage")]
    while start < stop {
        unsafe {
            *start = MAX_EDGES_FOUND as u32;
            start = start.offset(1);
        }

        #[cfg(feature = "pointer_maps")]
        {
            // SAFETY: we're the only ones accessing this static
            unsafe {
                MAX_EDGES_FOUND = MAX_EDGES_FOUND.wrapping_add(1) % EDGES_MAP_ALLOCATED_SIZE;
            }
        }
        #[cfg(not(feature = "pointer_maps"))]
        {
            let edges_map_ptr = &raw const EDGES_MAP;
            // SAFETY: we're the only ones accessing these statics
            unsafe {
                let edges_map_len = (*edges_map_ptr).len();
                MAX_EDGES_FOUND = MAX_EDGES_FOUND.wrapping_add(1);
                assert!(
                    MAX_EDGES_FOUND <= edges_map_len,
                    "The number of edges reported by SanitizerCoverage exceed the size of the edges map ({edges_map_len}). Use the LIBAFL_EDGES_MAP_DEFAULT_SIZE env to increase it at compile time."
                );
            }
        }
    }
}
