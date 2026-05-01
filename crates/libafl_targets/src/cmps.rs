use crate::{CMPLOG_MAP_H, CMPLOG_MAP_W};
use core::{
    fmt::{self, Debug, Formatter},
    mem::{size_of, zeroed},
};
use libafl::observers::{CmpMap, CmpValues, CmplogBytes};
use libafl_bolts::Error;

// CONSTANTS

/// The `CmpLog` map size
pub const CMPLOG_MAP_SIZE: usize = CMPLOG_MAP_W * CMPLOG_MAP_H;

/// The size of a logged routine argument in bytes
pub const CMPLOG_RTN_LEN: usize = 32;

/// The hight of a cmplog routine map
pub const CMPLOG_MAP_RTN_H: usize =
    (CMPLOG_MAP_H * size_of::<CmpLogInstruction>()) / size_of::<CmpLogRoutine>();

/// `CmpLog` instruction kind
pub const CMPLOG_KIND_INS: u8 = 0;
/// `CmpLog` routine kind
pub const CMPLOG_KIND_RTN: u8 = 1;

// EXTERNS, GLOBALS

#[cfg(any(
    feature = "cmplog",
    feature = "sancov_cmplog",
    feature = "sancov_value_profile"
))]
// void __libafl_targets_cmplog_instructions(uintptr_t k, uint8_t size, uint64_t arg1, uint64_t arg2)
unsafe extern "C" {
    /// Logs an instruction for feedback during fuzzing
    pub fn __libafl_targets_cmplog_instructions(k: usize, size: u8, arg1: u64, arg2: u64);

    /// Logs an AFL++ style instruction for feedback during fuzzing
    pub fn __libafl_targets_cmplog_instructions_extended(k: usize, size: u8, arg1: u64, arg2: u64);

    /// Logs a routine for feedback during fuzzing
    pub fn __libafl_targets_cmplog_routines(k: usize, ptr1: *const u8, ptr2: *const u8);

    /// Cmplog routines but with len specified.
    pub fn __libafl_targets_cmplog_routines_len(
        k: usize,
        ptr1: *const u8,
        ptr2: *const u8,
        len: usize,
    );

    /// Pointer to the `CmpLog` map
    pub static mut libafl_cmplog_map_ptr: *mut CmpLogMap;

    /// Pointer to the extended `CmpLog` map
    pub static mut libafl_cmplog_map_extended_ptr: *mut CmpLogMap;
}

#[cfg(feature = "cmplog")]
pub use libafl_cmplog_map_ptr as CMPLOG_MAP_PTR;

/// Value indicating if cmplog is enabled.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)] // expect breaks here for some reason
pub static mut libafl_cmplog_enabled: u8 = 0;

pub use libafl_cmplog_enabled as CMPLOG_ENABLED;

// HEADERS

/// The header for `CmpLog` hits.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct CmpLogHeader {
    hits: u16,
    shape: u8,
    kind: u8,
}

// VALS

/// The operands logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct CmpLogInstruction(u64, u64, u8);

/// The routine arguments logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct CmpLogRoutine([u8; CMPLOG_RTN_LEN], [u8; CMPLOG_RTN_LEN]);

/// Union of cmplog operands and routines
#[repr(C)]
#[derive(Copy, Clone)]
pub union CmpLogVals {
    operands: [[CmpLogInstruction; CMPLOG_MAP_H]; CMPLOG_MAP_W],
    routines: [[CmpLogRoutine; CMPLOG_MAP_RTN_H]; CMPLOG_MAP_W],
}

impl Debug for CmpLogVals {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CmpLogVals").finish_non_exhaustive()
    }
}

// MAPS

/// A struct containing the `CmpLog` metadata for a `LibAFL` run.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CmpLogMap {
    headers: [CmpLogHeader; CMPLOG_MAP_W],
    vals: CmpLogVals,
}

impl Default for CmpLogMap {
    fn default() -> Self {
        unsafe { zeroed() }
    }
}

impl CmpMap for CmpLogMap {
    fn len(&self) -> usize {
        CMPLOG_MAP_W
    }

    fn executions_for(&self, idx: usize) -> usize {
        self.headers[idx].hits as usize
    }

    fn usable_executions_for(&self, idx: usize) -> usize {
        if self.headers[idx].kind == CMPLOG_KIND_INS {
            if self.executions_for(idx) < CMPLOG_MAP_H {
                self.executions_for(idx)
            } else {
                CMPLOG_MAP_H
            }
        } else if self.executions_for(idx) < CMPLOG_MAP_RTN_H {
            self.executions_for(idx)
        } else {
            CMPLOG_MAP_RTN_H
        }
    }

    fn values_of(&self, idx: usize, execution: usize) -> Option<CmpValues> {
        if self.headers[idx].kind == CMPLOG_KIND_INS {
            let shape = self.headers[idx].shape;
            unsafe {
                match shape {
                    0 => Some(CmpValues::U8((
                        self.vals.operands[idx][execution].0 as u8,
                        self.vals.operands[idx][execution].1 as u8,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    1 => Some(CmpValues::U16((
                        self.vals.operands[idx][execution].0 as u16,
                        self.vals.operands[idx][execution].1 as u16,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    3 => Some(CmpValues::U32((
                        self.vals.operands[idx][execution].0 as u32,
                        self.vals.operands[idx][execution].1 as u32,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    7 => Some(CmpValues::U64((
                        self.vals.operands[idx][execution].0,
                        self.vals.operands[idx][execution].1,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    // TODO handle 128 bits & 256 bits & 512 bits cmps
                    15 | 31 | 63 => None,
                    _ => panic!("Invalid CmpLog shape {shape}"),
                }
            }
        } else {
            unsafe {
                Some(CmpValues::Bytes((
                    CmplogBytes::from_buf_and_len(
                        self.vals.routines[idx][execution].0,
                        CMPLOG_RTN_LEN as u8,
                    ),
                    CmplogBytes::from_buf_and_len(
                        self.vals.routines[idx][execution].1,
                        CMPLOG_RTN_LEN as u8,
                    ),
                )))
            }
        }
    }

    fn reset(&mut self) -> Result<(), Error> {
        // For performance, we reset just the headers
        self.headers.fill(CmpLogHeader {
            hits: 0,
            shape: 0,
            kind: 0,
        });

        Ok(())
    }
}

/// The global `CmpLog` map for the current `LibAFL` run.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)] // expect breaks here for some reason
pub static mut libafl_cmplog_map: CmpLogMap = CmpLogMap {
    headers: [CmpLogHeader {
        hits: 0,
        shape: 0,
        kind: 0,
    }; CMPLOG_MAP_W],
    vals: CmpLogVals {
        operands: [[CmpLogInstruction(0, 0, 0); CMPLOG_MAP_H]; CMPLOG_MAP_W],
    },
};
