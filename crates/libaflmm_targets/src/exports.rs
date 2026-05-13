//! target exports

pub use crate::cmps::libafl_cmplog_map as CMPLOG_MAP;

pub use crate::coverage::__afl_area_ptr as EDGES_MAP_PTR;
pub use crate::coverage::__afl_area_ptr_local as EDGES_MAP;
pub use crate::coverage::__afl_fuzz_len as INPUT_LENGTH_PTR;
pub use crate::coverage::__afl_fuzz_ptr as INPUT_PTR;
pub use crate::coverage::__afl_sharedmem_fuzzing as SHM_FUZZING;

#[cfg(feature = "cmplog")]
pub use crate::cmps::libafl_cmplog_map_ptr as CMPLOG_MAP_PTR;
pub use crate::libafl_cmplog_enabled as CMPLOG_ENABLED;
pub use crate::sancov::sancov_value_profile::libafl_cmp_map as CMP_MAP;
