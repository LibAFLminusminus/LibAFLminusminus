use crate::constants::{CMPLOG_MAP_H, CMPLOG_MAP_W};
use core::{
    fmt::{self, Debug, Formatter},
    mem::{size_of, zeroed},
    ops::Index,
};
use libaflmm_bolts::OwnedRefMut;
use libaflmm_bolts::{
    Error,
    shm::{EmptyShmHeader, SysVShm},
};

/// The standard cmplog header
pub type StdCmpLogHeader = LibAFLCmpLogHeader;

/// The standard cmplog values
pub type StdCmpLogVals = LibAFLCmpLogVals;

/// the standard cmplog map used by libaflmm instrumentation
pub type StdCmpLogMap = CmpLogMap<StdCmpLogHeader, StdCmpLogVals>;

// CONSTANTS

/// The `CmpLog` map size
pub const CMPLOG_MAP_SIZE: usize = CMPLOG_MAP_W * CMPLOG_MAP_H;

/// The size of a logged routine argument in bytes
pub const CMPLOG_RTN_LEN: usize = 32;

/// The hight of a cmplog routine map
pub const CMPLOG_MAP_RTN_H: usize =
    (CMPLOG_MAP_H * size_of::<LibAFLCmpLogInstruction>()) / size_of::<LibAFLCmpLogRoutine>();

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
// void __libaflmm_targets_cmplog_instructions(uintptr_t k, uint8_t size, uint64_t arg1, uint64_t arg2)
unsafe extern "C" {
    /// Logs an instruction for feedback during fuzzing
    pub fn __libaflmm_targets_cmplog_instructions(k: usize, size: u8, arg1: u64, arg2: u64);

    /// Logs an AFL++ style instruction for feedback during fuzzing
    pub fn __libaflmm_targets_cmplog_instructions_extended(
        k: usize,
        size: u8,
        arg1: u64,
        arg2: u64,
    );

    /// Logs a routine for feedback during fuzzing
    pub fn __libaflmm_targets_cmplog_routines(k: usize, ptr1: *const u8, ptr2: *const u8);

    /// Cmplog routines but with len specified.
    pub fn __libaflmm_targets_cmplog_routines_len(
        k: usize,
        ptr1: *const u8,
        ptr2: *const u8,
        len: usize,
    );

    /// Pointer to the `CmpLog` map
    pub static mut libafl_cmplog_map_ptr: *mut CmpLogMap<LibAFLCmpLogHeader, LibAFLCmpLogVals>;
}

/// Value indicating if cmplog is enabled.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)] // expect breaks here for some reason
pub static mut libafl_cmplog_enabled: u8 = 0;

// HEADERS
/// trait for cmplog header
pub trait CmpLogHeader: Clone + Default {
    /// number of times that this comparison was hit
    fn hits(&self) -> u16;
    /// the size of the comparison (in u8)
    fn kind(&self) -> u8;
    /// the size of the comparison (in u8)
    fn shape(&self) -> u8;
}

/// Trait for cmplog argument
pub trait Operand {
    /// first arg
    fn v0(&self) -> u64;
    /// second arg
    fn v1(&self) -> u64;
    /// auxilliary info
    fn aux(&self) -> u64;
}

/// trait for cmplog fn argument
pub trait Routine {
    /// first arg
    fn v0(&self) -> &[u8; CMPLOG_RTN_LEN];
    /// second arg
    fn v1(&self) -> &[u8; CMPLOG_RTN_LEN];
}

/// trait fro cmplog vals
pub trait CmpLogVals {
    /// operand in cmp
    type Operand: Operand;
    /// routine in cmp functions
    type Routine: Routine;
    /// operands in a row
    type OperandRow: Index<usize, Output = Self::Operand>;
    /// routines in a row
    type RoutineRow: Index<usize, Output = Self::Routine>;

    /// operands in cmp
    fn operands(&self) -> &[Self::OperandRow];
    /// operands in cmp functions
    fn routines(&self) -> &[Self::RoutineRow];
}

// AFLpp ZONE Start
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
/// the header used by aflpp
pub struct AFLppLibAFLCmpLogHeader(u16);

impl CmpLogHeader for AFLppLibAFLCmpLogHeader {
    fn hits(&self) -> u16 {
        self.0 & 0x3F
    }

    fn kind(&self) -> u8 {
        ((self.0 >> 11) & 0x1) as u8
    }

    fn shape(&self) -> u8 {
        ((self.0 >> 6) & 0x1F) as u8
    }
}

/// The operands logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
#[allow(missing_docs)]
pub struct AFLppCmpLogInstruction {
    pub v0: u64,
    pub v0_128: u64,
    pub v0_256_0: u64,
    pub v0_256_1: u64,
    pub v1: u64,
    pub v1_128: u64,
    pub v1_256_0: u64,
    pub v1_256_1: u64,
    pub unused: [u8; 8],
}

impl Operand for AFLppCmpLogInstruction {
    fn v0(&self) -> u64 {
        self.v0
    }

    fn v1(&self) -> u64 {
        self.v1
    }

    fn aux(&self) -> u64 {
        0
    }
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
#[allow(missing_docs)]
pub struct AFLppCmpLogRoutine {
    pub v0: [u8; CMPLOG_RTN_LEN],
    pub v1: [u8; CMPLOG_RTN_LEN],
    pub v0_len: u8,
    pub v1_len: u8,
    pub addr_attr: u8,
    pub unused: [u8; 5],
}

impl Routine for AFLppCmpLogRoutine {
    fn v0(&self) -> &[u8; CMPLOG_RTN_LEN] {
        &self.v0
    }

    fn v1(&self) -> &[u8; CMPLOG_RTN_LEN] {
        &self.v1
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
/// Union of cmplog operands and routines for aflpp
pub union AFLppCmplogVals {
    /// the value compared
    pub operands: [[AFLppCmpLogInstruction; CMPLOG_MAP_H]; CMPLOG_MAP_W],
    /// the function args compared
    pub fn_operands: [[AFLppCmpLogRoutine; CMPLOG_MAP_H]; CMPLOG_MAP_W],
}

impl CmpLogVals for AFLppCmplogVals {
    type Operand = AFLppCmpLogInstruction;
    type Routine = AFLppCmpLogRoutine;
    type OperandRow = [Self::Operand; CMPLOG_MAP_H];
    type RoutineRow = [Self::Routine; CMPLOG_MAP_H];

    fn operands(&self) -> &[Self::OperandRow] {
        unsafe { &self.operands }
    }

    fn routines(&self) -> &[Self::RoutineRow] {
        unsafe { &self.fn_operands }
    }
}

impl Debug for AFLppCmplogVals {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AFLppCmplogVals").finish_non_exhaustive()
    }
}

// AFLpp ZONE End

/// The header for `CmpLog` hits.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct LibAFLCmpLogHeader {
    /// number of times that this comparison was hit
    pub hits: u16,
    /// the size of the comparison (in u8)
    pub shape: u8,
    /// if it is insn or rtn
    pub kind: u8,
}

impl CmpLogHeader for LibAFLCmpLogHeader {
    fn hits(&self) -> u16 {
        self.hits
    }

    fn kind(&self) -> u8 {
        self.kind
    }

    fn shape(&self) -> u8 {
        self.shape
    }
}
// VALS
/// The operands logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct LibAFLCmpLogInstruction(pub u64, pub u64, pub u8);

impl Operand for LibAFLCmpLogInstruction {
    fn v0(&self) -> u64 {
        self.0
    }

    fn v1(&self) -> u64 {
        self.1
    }

    fn aux(&self) -> u64 {
        u64::from(self.2)
    }
}

/// The routine arguments logged during `CmpLog`.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct LibAFLCmpLogRoutine(pub [u8; CMPLOG_RTN_LEN], pub [u8; CMPLOG_RTN_LEN]);

impl Routine for LibAFLCmpLogRoutine {
    fn v0(&self) -> &[u8; CMPLOG_RTN_LEN] {
        &self.0
    }

    fn v1(&self) -> &[u8; CMPLOG_RTN_LEN] {
        &self.1
    }
}

/// Union of cmplog operands and routines
#[repr(C)]
#[derive(Copy, Clone)]
pub union LibAFLCmpLogVals {
    /// the value compared
    pub operands: [[LibAFLCmpLogInstruction; CMPLOG_MAP_H]; CMPLOG_MAP_W],
    /// the function args compared
    pub routines: [[LibAFLCmpLogRoutine; CMPLOG_MAP_RTN_H]; CMPLOG_MAP_W],
}

impl CmpLogVals for LibAFLCmpLogVals {
    type Operand = LibAFLCmpLogInstruction;
    type Routine = LibAFLCmpLogRoutine;
    type OperandRow = [Self::Operand; CMPLOG_MAP_H];
    type RoutineRow = [Self::Routine; CMPLOG_MAP_RTN_H];
    fn operands(&self) -> &[Self::OperandRow] {
        unsafe { &self.operands }
    }

    fn routines(&self) -> &[Self::RoutineRow] {
        unsafe { &self.routines }
    }
}

impl Debug for LibAFLCmpLogVals {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CmpLogVals").finish_non_exhaustive()
    }
}

// MAPS

/// A struct containing the `CmpLog` metadata for a `LibAFL` run.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CmpLogMap<H, V> {
    /// the headers to say what the values compared are
    pub headers: [H; CMPLOG_MAP_W],
    /// the acutual values
    pub vals: V,
}

impl<H, V> Default for CmpLogMap<H, V> {
    fn default() -> Self {
        unsafe { zeroed() }
    }
}

impl<H, V> CmpLogMap<H, V>
where
    H: CmpLogHeader,
{
    /// turn a sysv shared memory region as a cmplog map
    pub fn from_shm(shm: &mut SysVShm<EmptyShmHeader>) -> Result<OwnedRefMut<'_, Self>, Error> {
        let needed = size_of::<Self>();
        let available = shm.max_data_len();
        if available != needed {
            return Err(Error::illegal_argument(
                "Shmem size mismatch! You must provide a shm with an identical size!!",
            ));
        }
        let ptr = shm.shm_mut();
        let ptr = unsafe { ptr.data_mut().as_mut_ptr().cast::<Self>() };
        Ok(unsafe { OwnedRefMut::from_mut_ptr(ptr) })
    }

    /// length of this map
    #[must_use]
    pub const fn len(&self) -> usize {
        CMPLOG_MAP_W
    }

    /// whether this map is empty
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        CMPLOG_MAP_W == 0
    }

    /// how many cmps were recorded for this
    #[must_use]
    pub fn executions_for(&self, idx: usize) -> usize {
        self.headers[idx].hits() as usize
    }

    /// executions for but capped
    #[must_use]
    pub fn usable_executions_for(&self, idx: usize) -> usize {
        if self.headers[idx].kind() == CMPLOG_KIND_INS {
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
        self.headers.fill(H::default());

        Ok(())
    }
}

/// The global `CmpLog` map for the current `LibAFL` run.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)] // expect breaks here for some reason
pub static mut libafl_cmplog_map: CmpLogMap<LibAFLCmpLogHeader, LibAFLCmpLogVals> = CmpLogMap {
    headers: [LibAFLCmpLogHeader {
        hits: 0,
        shape: 0,
        kind: 0,
    }; CMPLOG_MAP_W],
    vals: LibAFLCmpLogVals {
        operands: [[LibAFLCmpLogInstruction(0, 0, 0); CMPLOG_MAP_H]; CMPLOG_MAP_W],
    },
};
