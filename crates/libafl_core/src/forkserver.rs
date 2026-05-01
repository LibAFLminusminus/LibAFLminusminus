//! Forkserver constants, shared between libafl and libafl_targets

/// Pinned fd number for forkserver communication
pub const FORKSRV_FD_NUM: i32 = 198;
/// New error
pub const FS_NEW_ERROR: i32 = 0xeffe0000_u32 as i32;

/// Minimum number for new version
pub const FS_NEW_VERSION_MIN: u32 = 1;
/// Maximum number for new version
pub const FS_NEW_VERSION_MAX: u32 = 1;
/// Base magic value for new-protocol forkserver status ("AFL\0")
pub const FS_NEW_VERSION_MAGIC_BASE: u32 = 0x41464c00;
/// Upper bound (inclusive) for new-protocol forkserver status
pub const FS_NEW_VERSION_MAGIC_MAX: u32 = 0x41464cff;

/// Whether forkserver option customization for old forkserver is enabled
#[expect(clippy::cast_possible_wrap)]
pub const FS_OPT_ENABLED: i32 = 0x80000001_u32 as i32;

/// Set map size option for new forkserver
#[expect(clippy::cast_possible_wrap)]
pub const FS_NEW_OPT_MAPSIZE: i32 = 1_u32 as i32;
/// Set map size option for old forkserver
#[expect(clippy::cast_possible_wrap)]
pub const FS_OPT_MAPSIZE: i32 = 0x40000000_u32 as i32;

/// Enable shared memory fuzzing option for old forkserver
#[expect(clippy::cast_possible_wrap)]
pub const FS_OPT_SHDMEM_FUZZ: i32 = 0x01000000_u32 as i32;
/// Enable shared memory fuzzing option for new forkserver
#[expect(clippy::cast_possible_wrap)]
pub const FS_NEW_OPT_SHDMEM_FUZZ: i32 = 2_u32 as i32;

/// Enable autodict option for new forkserver
#[expect(clippy::cast_possible_wrap)]
pub const FS_NEW_OPT_AUTODTCT: i32 = 0x00000800_u32 as i32;
/// Enable autodict option for old forkserver
#[expect(clippy::cast_possible_wrap)]
pub const FS_OPT_AUTODTCT: i32 = 0x10000000_u32 as i32;

/// Failed to set map size
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_MAP_SIZE: i32 = 1_u32 as i32;
/// Failed to map address
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_MAP_ADDR: i32 = 2_u32 as i32;
/// Failed to open shared memory
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_SHM_OPEN: i32 = 4_u32 as i32;
/// Failed to do `shmat`
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_SHMAT: i32 = 8_u32 as i32;
/// Failed to do `mmap`
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_MMAP: i32 = 16_u32 as i32;
/// Old cmplog error
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_OLD_CMPLOG: i32 = 32_u32 as i32;
/// Old QEMU cmplog error
#[expect(clippy::cast_possible_wrap)]
pub const FS_ERROR_OLD_CMPLOG_QEMU: i32 = 64_u32 as i32;
/// Flag indicating this is an error
#[expect(clippy::cast_possible_wrap)]
pub const FS_OPT_ERROR: i32 = 0xf800008f_u32 as i32;
