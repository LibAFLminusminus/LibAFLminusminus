//! A simple typed shared memory implementation
//! It is mostly boilerplate code to use in libafl for various shared-memory based operations.

use core::{ptr::NonNull, slice, sync::atomic::Ordering};

use atomic::Atomic;
use libafl_core::{Result, runtime};
use num_traits::{Bounded, NumCast};
use wide::bytemuck::NoUninit;

pub mod anonymous;
pub use anonymous::{AnonShmBuilder, AnonShmReceiver, AnonShmSender};

pub mod sysv;
pub use sysv::SysVShm;

/// The magic value signaling the shared memory value is invalid.
pub fn invalid_shm_size<SZ: NumCast + Bounded>() -> SZ {
    SZ::max_value()
}

/// A piece of shared memory
///
/// It must be created using one of the implemented shared memory models (System V, POSIX, etc...)
/// It has the following layout in memory:
///
/// |                 size                  |
/// |                                       |
/// <-- real_size --><-------- data -------->
/// |                |                      |
/// |  size_of<SZ>() | size - size_of<SZ>() |
#[derive(Debug)]
pub struct SharedMemory<SZ: NoUninit> {
    ptr: NonNull<u8>,
    size: Atomic<SZ>,
}

impl<SZ: NoUninit> Clone for SharedMemory<SZ> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
            size: Atomic::new(self.size.load(Ordering::Relaxed)),
        }
    }
}

impl<SZ> SharedMemory<SZ>
where
    SZ: NoUninit + NumCast + Bounded + PartialEq,
{
    /// Create a new shared memory section.
    ///
    /// # Safety
    ///
    /// Of course, the ptr and size should be valid shared memory.
    ///
    /// `size` should be the total size of the underlying shared memory.
    /// Be careful, as `size` includes the size taken by the "real" size field in the first few bytes.
    pub unsafe fn new(ptr: NonNull<u8>, size: SZ) -> Result<Self> {
        let size_atomic = Atomic::new(size);
        let size_usize: usize = NumCast::from(size).unwrap();

        if size_usize < size_of::<SZ>() {
            return Err(runtime!(
                "Shared memory region is too smal: {} bytes",
                size_usize
            ));
        }

        let mut shm = Self {
            ptr,
            size: size_atomic,
        };

        // safety guard: start start with invalid value.
        shm.mark_invalid();

        Ok(shm)
    }

    /// # Safety
    ///
    /// This MUST be called after set_size has been called on the shared memory
    pub unsafe fn data(&self) -> &[u8] {
        let hdr_size = size_of::<SZ>();
        let size = self.get_size().expect("Invalid data size stored.");
        let size_usize = NumCast::from(size).unwrap();

        unsafe { slice::from_raw_parts(self.ptr.as_ptr().add(hdr_size), size_usize) }
    }

    /// Get the inner full shared memory data as mutable.
    ///
    /// # Safety
    ///
    /// The function [`set_size`] MUST be called after writing to the shared memory
    /// with the size effectively written.
    /// [`set_size`] (or [`mark_invalid`]) must be called before any other calls to [`Self`] after calling this function.
    pub unsafe fn data_mut(&mut self) -> &mut [u8] {
        let hdr_size = size_of::<SZ>();
        let size = self.size.load(Ordering::SeqCst);
        let size_usize: usize = NumCast::from(size).unwrap();

        unsafe {
            slice::from_raw_parts_mut(
                self.ptr.as_ptr().add(hdr_size),
                size_usize.checked_sub(hdr_size).unwrap(),
            )
        }
    }

    /// Set the size effectively written while manipulating data_mut.
    ///
    /// # Safety
    ///
    /// It MUST be set before reading data using [`data`] and after writing through [`data_mut`].
    /// The ONLY valid call to this function without writing is to write a size of 0.
    pub unsafe fn set_size(&mut self, size: SZ) {
        let size_ptr = self.ptr.as_ptr() as *mut Atomic<SZ>;
        unsafe { (*size_ptr).store(size, Ordering::SeqCst) }
    }

    /// return Some with the effective size, and None if the size is invalid.
    pub fn get_size(&self) -> Option<SZ> {
        let size_ptr = self.ptr.as_ptr() as *const Atomic<SZ>;
        let size = unsafe { (*size_ptr).load(Ordering::SeqCst) };

        if size == invalid_shm_size() {
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
            self.set_size(invalid_shm_size());
        }
    }
}
