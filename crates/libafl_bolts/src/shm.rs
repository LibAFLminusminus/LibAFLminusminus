//! A simple Shared Memory utility

use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use libafl_core::{Result, runtime, serialize};
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::slice;

/// The magic value signaling the shared memory value is invalid.
pub const INVALID_SHM_SIZE: usize = usize::MAX;

/// A piece of shared memory
#[derive(Debug, Clone)]
pub struct OsSharedMemory {
    ptr: NonNull<u8>,
    size: NonZeroUsize,
}

/// A helper builder
///
/// [`OsShmSender`] and [`OsShmReceiver`] should be built from here.
#[derive(Debug)]
pub struct OsShmBuilder;

/// A shared memory sender
///
/// It sends some value on the shared memory,
/// ready to be consumed by the [`OsShmReceiver`]
#[derive(Debug)]
pub struct OsShmSender<T> {
    shm: OsSharedMemory,
    phantom: PhantomData<T>,
}

/// A shared memory receiver
///
/// It receives memory sent by the [`OsShmSender`].
#[derive(Debug)]
pub struct OsShmReceiver<T> {
    shm: OsSharedMemory,
    phantom: PhantomData<T>,
}

impl OsSharedMemory {
    /// Create a new shared memory section.
    ///
    /// # Safety
    ///
    /// Of course, the ptr and size should be valid shared memory.
    pub unsafe fn new(ptr: NonNull<u8>, size: NonZeroUsize) -> Result<Self> {
        let mut shm = Self { ptr, size };

        if size.get() < size_of::<AtomicUsize>() {
            return Err(runtime!(
                "Shared memory region is too smal: {} bytes",
                size.get()
            ));
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
        let hdr_size = size_of::<AtomicUsize>();
        let size = self.get_size().expect("Invalid data size stored.");

        unsafe { slice::from_raw_parts(self.ptr.as_ptr().add(hdr_size), size) }
    }

    /// Get the inner full shared memory data as mutable.
    ///
    /// # Safety
    ///
    /// The function [`set_size`] MUST be called after writing to the shared memory
    /// with the size effectively written.
    /// [`set_size`] (or [`mark_invalid`]) must be called before any other calls to [`Self`] after calling this function.
    pub unsafe fn data_mut(&mut self) -> &mut [u8] {
        let hdr_size = size_of::<AtomicUsize>();

        unsafe {
            slice::from_raw_parts_mut(
                self.ptr.as_ptr().add(hdr_size),
                self.size.get().checked_sub(hdr_size).unwrap(),
            )
        }
    }

    /// Set the size effectively written while manipulating data_mut.
    ///
    /// # Safety
    ///
    /// It MUST be set before reading data using [`data`] and after writing through [`data_mut`].
    /// The ONLY valid call to this function without writing is to write a size of 0.
    pub unsafe fn set_size(&mut self, size: usize) {
        let size_ptr = self.ptr.as_ptr() as *mut AtomicUsize;
        unsafe { (*size_ptr).store(size, Ordering::SeqCst) }
    }

    /// return Some with the effective size, and None if the size is invalid.
    pub fn get_size(&self) -> Option<usize> {
        let size_ptr = self.ptr.as_ptr() as *const AtomicUsize;
        let size = unsafe { (*size_ptr).load(Ordering::SeqCst) };

        if size == INVALID_SHM_SIZE {
            None
        } else {
            Some(size)
        }
    }

    /// Is there a valid shared data available?
    pub fn is_invalid(&self) -> bool {
        self.get_size().is_none()
    }

    /// Mark the shared data as invalid.
    pub fn mark_invalid(&mut self) {
        unsafe {
            self.set_size(INVALID_SHM_SIZE);
        }
    }
}

impl OsShmBuilder {
    /// Build a pair of shared data sender / receiver.
    /// The sender writes the data to share, and the receiver handles it.
    /// Once data is received, the shared memory is marked as invalid until the sender sends something new.
    pub fn build<T>(max_data_size: NonZeroUsize) -> Result<(OsShmSender<T>, OsShmReceiver<T>)> {
        let shared_memory = unsafe {
            mmap_anonymous(
                None,
                max_data_size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
            )
            .expect("Could not allocate shared memory for the shm sender")
        };

        unsafe {
            let shm = OsSharedMemory::new(shared_memory.cast(), max_data_size)?;

            Ok((OsShmSender::new(shm.clone()), OsShmReceiver::new(shm)))
        }
    }
}

impl<T> OsShmSender<T> {
    unsafe fn new(shm: OsSharedMemory) -> Self {
        Self {
            shm,
            phantom: PhantomData,
        }
    }
}

impl<T> OsShmReceiver<T> {
    unsafe fn new(shm: OsSharedMemory) -> Self {
        Self {
            shm,
            phantom: PhantomData,
        }
    }
}

impl<T> OsShmSender<T>
where
    T: Serialize,
{
    /// Send some data on the shared memory
    ///
    /// It should be received by the paired [`OsShmReceiver`].
    pub fn send(&mut self, data: &T) -> Result<()> {
        let data_shm = unsafe { self.shm.data_mut() };

        let used_len = match postcard::to_slice(data, data_shm) {
            Ok(used_slice) => used_slice.len(),
            Err(e) => {
                return Err(serialize!("Error while serializing data: {e}"));
            }
        };

        unsafe {
            self.shm.set_size(used_len);
        }

        Ok(())
    }
}

impl<T> OsShmReceiver<T>
where
    for<'de> T: Deserialize<'de>,
{
    /// # Safety
    ///
    /// OsShmSender::send must be called BEFORE calling this function
    ///
    /// There is no synchronization in place.
    /// You are responsible to synchronizing save and store correctly.
    /// A validation check is performed before giving back the data sent.
    pub unsafe fn receive(&mut self) -> Result<T> {
        if self.shm.is_invalid() {
            return Err(runtime!(
                "Trying to restore from an invalid shared memory. The data has most likely not been saved correctly.",
            ));
        }

        let value_shm = unsafe { self.shm.data() };
        let value = postcard::from_bytes::<T>(value_shm)
            .map_err(|e| serialize!("Error while deserializing value: {e}."))?;

        self.shm.mark_invalid();

        Ok(value)
    }
}
