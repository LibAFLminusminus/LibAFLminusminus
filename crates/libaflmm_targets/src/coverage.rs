//! Coverage maps as static mut array

use crate::{
    EDGES_MAP_ALLOCATED_SIZE, EDGES_MAP_DEFAULT_SIZE,
    exports::{EDGES_MAP, EDGES_MAP_PTR},
};

/// The map for edges.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)] // expect breaks here for some reason
pub static mut __afl_area_ptr_local: [u8; EDGES_MAP_ALLOCATED_SIZE] = [0; EDGES_MAP_ALLOCATED_SIZE];

/// The max count of edges found.
///
/// This is either computed during the compilation time or at runtime (in this case this is used to shrink the map).
/// You can use this for the initial map size for the observer only if you compute this time at compilation time.
pub static mut MAX_EDGES_FOUND: usize = 0;

unsafe extern "C" {
    /// The sharedmemort fuzzing flag
    pub static mut __afl_sharedmem_fuzzing: core::ffi::c_uint;

    /// The area pointer points to the edges map.
    pub static mut __afl_area_ptr: *mut u8;

    /// The area pointer points to the accounting mem operations map.
    pub static mut __afl_acc_memop_ptr: *mut u32;

    /// Start of libafl token section
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    pub static __token_start: *const u8;

    /// End of libafl token section
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    pub static __token_stop: *const u8;
}
/// Check if we have enabled autotokens
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
pub(crate) fn has_autotokens() -> bool {
    unsafe {
        !__token_start.is_null()
            && !__token_stop.is_null()
            && __token_stop.offset_from(__token_start) != 0
    }
}

/// Return Tokens from the compile-time token section
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
#[must_use]
pub fn autotokens() -> Option<(*const u8, *const u8)> {
    // # Safety
    // All values are checked before dereferencing.
    unsafe {
        if has_autotokens() {
            // we can safely unwrap
            Some((__token_start, __token_stop))
        } else {
            None
        }
    }
}

/// The actual size we use for the map of edges.
/// This is used for forkserver backend
#[allow(non_upper_case_globals)] // expect breaks here for some reason
#[unsafe(no_mangle)]
pub static mut __afl_map_size: usize = EDGES_MAP_DEFAULT_SIZE;
/// The pointer points to the AFL++ inputs
#[allow(non_upper_case_globals)] // expect breaks here for some reason
#[unsafe(no_mangle)]
pub static mut __afl_fuzz_ptr: *mut u8 = core::ptr::null_mut();
#[allow(non_upper_case_globals)] // expect breaks here for some reason
static mut __afl_fuzz_len_local: u32 = 0;
/// The pointer points to the length of AFL++ inputs
#[allow(non_upper_case_globals)] // expect breaks here for some reason
#[unsafe(no_mangle)]
pub static mut __afl_fuzz_len: *mut u32 = &raw mut __afl_fuzz_len_local;

#[cfg(any(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts",))]
use libaflmm_bolts::ownedref::OwnedMutSlice;

/// Gets the edges map from the `EDGES_MAP_PTR` raw pointer.
/// Assumes a `len` of at least `EDGES_MAP_PTR_MAX`.
///
/// # Safety
///
/// This function will crash if `edges_map_mut_ptr` is not a valid pointer.
/// The [`edges_max_num`] needs to be smaller than, or equal to the size of the map.
#[must_use]
#[cfg(any(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts",))]
pub unsafe fn edges_map_mut_slice<'a>() -> OwnedMutSlice<'a, u8> {
    unsafe { OwnedMutSlice::from_raw_parts_mut(edges_map_mut_ptr(), edges_max_num()) }
}

/// Gets the current edges map pt
/// It will usually take `EDGES_MAP`, but `EDGES_MAP_PTR`,
/// if built with the `pointer_maps` feature.
#[must_use]
pub fn edges_map_mut_ptr() -> *mut u8 {
    unsafe {
        if cfg!(feature = "pointer_maps") {
            assert!(!EDGES_MAP_PTR.is_null());
            EDGES_MAP_PTR
        } else {
            &raw mut EDGES_MAP as *mut u8
        }
    }
}

/// Gets the current maximum number of edges tracked.
#[cfg(any(feature = "sancov_pcguard_edges", feature = "sancov_pcguard_hitcounts",))]
#[must_use]
pub fn edges_max_num() -> usize {
    unsafe {
        if MAX_EDGES_FOUND > 0 {
            MAX_EDGES_FOUND
        } else {
            #[cfg(feature = "pointer_maps")]
            {
                EDGES_MAP_ALLOCATED_SIZE // the upper bound
            }
            #[cfg(not(feature = "pointer_maps"))]
            {
                let edges_map_ptr = &raw const EDGES_MAP;
                (*edges_map_ptr).len()
            }
        }
    }
}
