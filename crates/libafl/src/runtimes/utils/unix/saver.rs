use core::marker::PhantomData;
use core::mem;
use core::ptr::{self, NonNull, slice_from_raw_parts_mut};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::num::NonZeroUsize;
use std::os::fd::{FromRawFd, IntoRawFd};
use std::slice;
use std::{fs::File, os::fd::OwnedFd};

use libafl_core::Error;
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use serde::{Deserialize, Serialize};

use crate::state::State;

pub const INVALID_SHM_SIZE: usize = usize::MAX;

#[derive(Debug, Clone)]
pub struct OsSharedMemory {
    ptr: NonNull<u8>,
    size: NonZeroUsize,
}

pub struct OsSaveRestoreBuilder;

pub struct OsSaver<S> {
    shm: OsSharedMemory,
    phantom: PhantomData<S>,
}

pub struct OsRestorer<S> {
    shm: OsSharedMemory,
    phantom: PhantomData<S>,
}

impl OsSharedMemory {
    pub unsafe fn new(ptr: NonNull<u8>, size: NonZeroUsize) -> Result<Self, Error> {
        let mut shm = Self { ptr, size };

        if size.get() < mem::size_of::<AtomicUsize>() {
            return Err(Error::runtime(format!(
                "Shared memory region is too smal: {} bytes",
                size.get()
            )));
        }

        // safety guard: start start with invalid value.
        unsafe {
            shm.set_size(INVALID_SHM_SIZE);
        }

        Ok(shm)
    }

    /// # Safety
    ///
    /// This MUST be called after set_size has been called on the shared memory
    pub unsafe fn data(&self) -> &[u8] {
        let hdr_size = mem::size_of::<AtomicUsize>();
        let size_ptr = self.ptr.as_ptr() as *mut AtomicUsize;
        let size = unsafe { (*size_ptr).load(Ordering::SeqCst) };

        unsafe { slice::from_raw_parts(self.ptr.as_ptr().add(hdr_size), size) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        let hdr_size = mem::size_of::<AtomicUsize>();

        unsafe {
            slice::from_raw_parts_mut(
                self.ptr.as_ptr().add(hdr_size),
                self.size.get().checked_sub(hdr_size).unwrap(),
            )
        }
    }

    /// # Safety
    ///
    /// Set the size effectively written while manipulating data_mut.
    /// It MUST be set before reading data using `data`.
    pub unsafe fn set_size(&mut self, size: usize) {
        let size_ptr = self.ptr.as_ptr() as *mut AtomicUsize;
        unsafe { (*size_ptr).store(size, Ordering::SeqCst) }
    }

    /// return None if the size is invalid.
    pub fn get_size(&self) -> Option<usize> {
        let size_ptr = self.ptr.as_ptr() as *const AtomicUsize;
        let size = unsafe { (*size_ptr).load(Ordering::SeqCst) };

        if size == INVALID_SHM_SIZE {
            None
        } else {
            Some(size)
        }
    }

    pub fn is_invalid(&self) -> bool {
        self.get_size().is_none()
    }

    pub fn mark_invalid(&mut self) {
        unsafe {
            self.set_size(INVALID_SHM_SIZE);
        }
    }
}

impl OsSaveRestoreBuilder {
    pub fn build<S>(max_state_size: NonZeroUsize) -> Result<(OsSaver<S>, OsRestorer<S>), Error> {
        let shared_memory = unsafe {
            mmap_anonymous(
                None,
                max_state_size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
            )
            .expect("Could not allocate shared memory for the state saver")
        };

        unsafe {
            let shm = OsSharedMemory::new(shared_memory.cast(), max_state_size)?;

            Ok((OsSaver::new(shm.clone()), OsRestorer::new(shm)))
        }
    }
}

impl<S> OsSaver<S> {
    unsafe fn new(shm: OsSharedMemory) -> Self {
        Self {
            shm,
            phantom: PhantomData,
        }
    }
}

impl<S> OsRestorer<S> {
    unsafe fn new(shm: OsSharedMemory) -> Self {
        Self {
            shm,
            phantom: PhantomData,
        }
    }
}

impl<S> OsSaver<S>
where
    S: Serialize,
{
    pub fn save(&mut self, state: &S) -> Result<(), Error> {
        let state_shm = self.shm.data_mut();

        let used_len = match postcard::to_slice(state, state_shm) {
            Ok(used_slice) => used_slice.len(),
            Err(e) => {
                return Err(Error::serialize(format!(
                    "Error while serializing state: {e}"
                )));
            }
        };

        unsafe {
            self.shm.set_size(used_len);
        }

        Ok(())
    }
}

impl<S> OsRestorer<S>
where
    for<'de> S: Deserialize<'de>,
{
    /// # Safety
    ///
    /// OsSaver::save must be called BEFORE calling this function
    /// There is no synchronization in place.
    /// You are responsible to synchronizing save and store correctly.
    pub unsafe fn restore(&mut self) -> Result<S, Error> {
        if self.shm.is_invalid() {
            return Err(Error::runtime(
                "Trying to restore from an invalid shared memory. The state has most likely not been saved correctly.",
            ));
        }

        let state_shm = unsafe { self.shm.data() };
        let state = postcard::from_bytes::<S>(state_shm)
            .map_err(|e| Error::serialize(format!("Error while deserializing state: {e}.")))?;

        self.shm.mark_invalid();

        Ok(state)
    }
}
