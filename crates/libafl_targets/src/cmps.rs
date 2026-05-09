use crate::{CMPLOG_MAP_H, CMPLOG_MAP_W};
use core::{
    fmt::{self, Debug, Formatter},
    mem::{size_of, zeroed},
};
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

/// Value indicating if cmplog is enabled.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)] // expect breaks here for some reason
pub static mut libafl_cmplog_enabled: u8 = 0;

// HEADERS

/// The header for `CmpLog` hits.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct CmpLogHeader {
    /// number of times that this comparison was hit
    pub hits: u16,
    /// the size of the comparison (in u8)
    pub shape: u8,
    /// if it is insn or rtn
    pub kind: u8,
}

// VALS

/// The operands logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct CmpLogInstruction(pub u64, pub u64, pub u8);

/// The routine arguments logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct CmpLogRoutine(pub [u8; CMPLOG_RTN_LEN], pub [u8; CMPLOG_RTN_LEN]);

/// Union of cmplog operands and routines
#[repr(C)]
#[derive(Copy, Clone)]
pub union CmpLogVals {
    /// the value compared
    pub operands: [[CmpLogInstruction; CMPLOG_MAP_H]; CMPLOG_MAP_W],
    /// the function args compared
    pub routines: [[CmpLogRoutine; CMPLOG_MAP_RTN_H]; CMPLOG_MAP_W],
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
    /// the headers to say what the values compared are
    pub headers: [CmpLogHeader; CMPLOG_MAP_W],
    /// the acutual values
    pub vals: CmpLogVals,
}

impl Default for CmpLogMap {
    fn default() -> Self {
        unsafe { zeroed() }
    }
}

impl CmpLogMap {
    /// length of this map
    pub fn len(&self) -> usize {
        CMPLOG_MAP_W
    }

    /// how many cmps were recorded for this
    pub fn executions_for(&self, idx: usize) -> usize {
        self.headers[idx].hits as usize
    }

    /// executions for but capped
    pub fn usable_executions_for(&self, idx: usize) -> usize {
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

    /// reset the map
    pub fn reset(&mut self) -> Result<(), Error> {
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
