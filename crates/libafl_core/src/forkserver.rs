//! Forkserver constants, shared between libafl and `libafl_targets`

/// Pinned fd number for forkserver communication
pub const FORKSRV_FD_NUM: i32 = 198;
/// New error
pub const FS_NEW_ERROR: i32 = 0xeffe0000_u32.cast_signed();

/// Ping @tokatoka if this changes
/// (72 * 32 + 2) * 65536
pub const AFLPP_CMPLOG_MAP: usize = 1_5112_6016;

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

/// The length of header bytes which tells shmem size
pub const SHMEM_FUZZ_HDR_SIZE: usize = 4;
/// Maximum default length for input
pub const MAX_INPUT_SIZE_DEFAULT: usize = 1024 * 1024;
/// Minimum default length for input
pub const MIN_INPUT_SIZE_DEFAULT: usize = 1;
/// Environment variable key for shared memory id for input and its len
pub const SHM_FUZZ_ENV_VAR: &str = "__AFL_SHM_FUZZ_ID";
/// Environment variable key for the page size (at least/usually `testcase_size_max + sizeof::<u32>()`)
pub const SHM_FUZZ_MAP_SIZE_ENV_VAR: &str = "__AFL_SHM_FUZZ_MAP_SIZE";

/// Environment variable key for shared memory id for edge map
pub const SHM_ENV_VAR: &str = "__AFL_SHM_ID";
/// Environment variable key for shared memory id for cmplog map
pub const SHM_CMPLOG_ENV_VAR: &str = "__AFL_CMPLOG_SHM_ID";

/// Environment variable key for a custom AFL coverage map size
pub const AFL_MAP_SIZE_ENV_VAR: &str = "AFL_MAP_SIZE";

/// Environment variable keys to skip instrumentation (LLVM variant).
pub const AFL_LLVM_ONLY_FSRV_VAR: &str = "AFL_LLVM_ONLY_FSRV";

/// Environment variable keys to skip instrumentation (GCC variant).
pub const AFL_GCC_ONLY_FSRV_VAR: &str = "AFL_GCC_ONLY_FSRV";
