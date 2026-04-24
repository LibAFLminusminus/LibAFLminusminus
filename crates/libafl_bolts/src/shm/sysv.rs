//! A simple system V shared memory model

use crate::shm::SharedMemory;
use core::{
    ffi::c_void,
    ptr::{self, NonNull},
};
use libafl_core::{Result, last_os_error};
use libc::{IPC_CREAT, IPC_EXCL, IPC_PRIVATE, key_t, shmat, shmget};
use num_traits::{Bounded, NumCast};
use std::string::{String, ToString};
use wide::bytemuck::NoUninit;

/// A simple abstraction over System V shared memory
#[derive(Debug)]
pub struct SysVShm<SZ: NoUninit> {
    shm: SharedMemory<SZ>,
    shm_id: key_t,
}

impl<SZ> SysVShm<SZ>
where
    SZ: NoUninit + NumCast + Bounded + PartialEq,
{
    /// Create a System V-backed shared memory region.
    /// max_size corresponds to the data size to store.
    pub fn new(max_size: SZ) -> Result<Self> {
        let max_size_usize: usize = NumCast::from(max_size).unwrap();
        let shm_size_usize = max_size_usize + size_of::<SZ>();
        let shm_size: SZ = NumCast::from(shm_size_usize).unwrap();

        let shm_id = unsafe { shmget(IPC_PRIVATE, shm_size_usize, IPC_CREAT | IPC_EXCL | 0o600) };

        if shm_id == -1 {
            return Err(last_os_error!("shmget failed"));
        }

        let shm_ptr = unsafe { shmat(shm_id, ptr::null(), 0) };

        if shm_ptr == -1isize as *mut c_void {
            return Err(last_os_error!("shmat failed"));
        }

        let shm_ptr = NonNull::new(shm_ptr.cast())
            .expect("Shared memory pointer should be non-null at this point");

        let shm = unsafe { SharedMemory::new(shm_ptr, shm_size)? };

        Ok(Self { shm, shm_id })
    }

    /// Get the string representation of the System V shared memory ID
    pub fn shm_id(&self) -> String {
        self.shm_id.to_string()
    }

    /// Get a reference to the underlying shared memory
    pub fn shm(&self) -> &SharedMemory<SZ> {
        &self.shm
    }

    /// Get a mutable ref to the underlying shared memory
    pub fn shm_mut(&mut self) -> &mut SharedMemory<SZ> {
        &mut self.shm
    }
}
