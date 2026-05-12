//! A simple system V shared memory model

use alloc::string::{String, ToString};
use core::{
    ffi::c_void,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
};
use std::env;

use libafl_core::{Result, last_os_error};
use libc::{IPC_CREAT, IPC_EXCL, IPC_PRIVATE, key_t, shmat, shmget};

use crate::{
    EmptyShmHeader,
    shm::{SharedMemory, ShmHeader},
};

/// A simple abstraction over System V shared memory
#[derive(Debug)]
pub struct SysVShm<SZ: ShmHeader> {
    shm: SharedMemory<SZ>,
    shm_id: key_t,
}

impl Deref for SysVShm<EmptyShmHeader> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.shm
    }
}

impl DerefMut for SysVShm<EmptyShmHeader> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.shm
    }
}

impl SysVShm<EmptyShmHeader> {
    /// Create a System V-backed shared memory region of `size` bytes.
    pub fn new(size: usize) -> Result<Self> {
        Self::new_with_hdr(size)
    }
}

impl<SZ: ShmHeader> SysVShm<SZ> {
    /// Create a System V-backed shared memory region.
    /// `max_size` is the maximum number of data bytes (not including the header).
    pub fn new_with_hdr(max_size: usize) -> Result<Self> {
        let total_size = max_size + SZ::HEADER_SIZE;

        let shm_id = unsafe { shmget(IPC_PRIVATE, total_size, IPC_CREAT | IPC_EXCL | 0o600) };

        if shm_id == -1 {
            return Err(last_os_error!("shmget failed"));
        }

        let shm_ptr = unsafe { shmat(shm_id, ptr::null(), 0) };

        if shm_ptr == -1isize as *mut c_void {
            return Err(last_os_error!("shmat failed"));
        }

        let shm_ptr = NonNull::new(shm_ptr.cast())
            .expect("Shared memory pointer should be non-null at this point");

        let shm = unsafe { SharedMemory::new(shm_ptr, total_size)? };

        Ok(Self { shm, shm_id })
    }

    /// Get the string representation of the System V shared memory ID
    #[must_use]
    pub fn shm_id(&self) -> String {
        self.shm_id.to_string()
    }

    /// Get a reference to the underlying shared memory
    #[must_use]
    pub fn shm(&self) -> &SharedMemory<SZ> {
        &self.shm
    }

    /// Total size of the underlying allocation (header (if any) + data).
    #[must_use]
    pub fn shm_size_usize(&self) -> usize {
        self.shm.total_len()
    }

    /// Get a mutable ref to the underlying shared memory
    pub fn shm_mut(&mut self) -> &mut SharedMemory<SZ> {
        &mut self.shm
    }

    /// Total len (including header size)
    #[must_use]
    pub fn total_len(&self) -> usize {
        self.shm.total_len()
    }

    /// Maximum data len (excluding header size)
    #[must_use]
    pub fn max_data_len(&self) -> usize {
        self.shm.max_data_len()
    }

    /// Write this map's config to env
    ///
    /// # Safety
    /// Writes to env variables and may only be done single-threaded.
    #[cfg(feature = "std")]
    pub unsafe fn write_to_env(&self, env_name: &str) -> Result<()> {
        let map_size = self.shm.total_len();
        let map_size_env = format!("{env_name}_SIZE");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { env::set_var(env_name, self.shm_id().clone()) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { env::set_var(map_size_env, format!("{map_size}")) };
        Ok(())
    }
}
