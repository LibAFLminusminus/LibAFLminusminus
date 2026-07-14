//! A simple typed shared memory implementation
//! It is mostly boilerplate code to use in libafl for various shared-memory based operations.
//!
//! It supports both header-based and header-less style shared memory.

use atomic::Atomic;
use bytemuck::NoUninit;
use core::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    slice,
    sync::atomic::Ordering,
};
use libaflmm_core::{Result, runtime};
use num_traits::{Bounded, NumCast};

pub mod anonymous;
pub use anonymous::{AnonShmBuilder, AnonShmReceiver, AnonShmSender};

pub mod sysv;
pub use sysv::SysVShm;

/// The invalid marker for a given memory region.
#[inline]
#[must_use]
pub fn invalid_shm_size<SZ: NumCast + Bounded>() -> SZ {
    SZ::max_value()
}

/// Describes the header layout of a [`SharedMemory`] region.
pub trait ShmHeader {
    /// The size of the header in bytes.
    const HEADER_SIZE: usize;

    /// Returns `Some(data_size)` when the region holds valid data, `None` when invalid.
    ///
    /// # Safety
    ///
    /// `ptr` must point to the start of a live shared memory region of at least
    /// `HEADER_SIZE + total_minus_header` bytes.
    unsafe fn read_real_size(ptr: NonNull<u8>, max_data_len: usize) -> Option<usize>;

    /// Stores `size` into the header.
    ///
    /// # Safety
    ///
    /// `ptr` must point to the start of a live shared memory region with at least
    /// `HEADER_SIZE` bytes.
    unsafe fn write_real_size(ptr: NonNull<u8>, size: usize);

    /// Marks the region as invalid by writing the sentinel value into the header.
    ///
    /// # Safety
    ///
    /// Same requirements as [`Self::write_real_size`].
    unsafe fn invalidate(ptr: NonNull<u8>);
}

/// Marks an SHM header as empty.
/// There will be no size header if use.
///
/// The shared memory is guaranteed to behave exactly like a usual
/// memory slice.
#[derive(Debug)]
pub struct EmptyShmHeader;

impl ShmHeader for EmptyShmHeader {
    const HEADER_SIZE: usize = 0;

    #[inline]
    unsafe fn read_real_size(_ptr: NonNull<u8>, total_minus_header: usize) -> Option<usize> {
        Some(total_minus_header)
    }

    #[inline]
    unsafe fn write_real_size(_ptr: NonNull<u8>, _size: usize) {}

    #[inline]
    unsafe fn invalidate(_ptr: NonNull<u8>) {}
}

impl<SZ> ShmHeader for SZ
where
    SZ: NoUninit + NumCast + Bounded + PartialEq,
{
    const HEADER_SIZE: usize = size_of::<SZ>();

    unsafe fn read_real_size(ptr: NonNull<u8>, _total_minus_header: usize) -> Option<usize> {
        let size_ptr = ptr.as_ptr() as *const Atomic<SZ>;
        let size = unsafe { (*size_ptr).load(Ordering::SeqCst) };

        if size == invalid_shm_size::<SZ>() {
            None
        } else {
            Some(NumCast::from(size).unwrap())
        }
    }

    unsafe fn write_real_size(ptr: NonNull<u8>, size: usize) {
        let sz: SZ = NumCast::from(size).unwrap();
        let size_ptr = ptr.as_ptr() as *mut Atomic<SZ>;

        unsafe { (*size_ptr).store(sz, Ordering::SeqCst) }
    }

    unsafe fn invalidate(ptr: NonNull<u8>) {
        let size_ptr = ptr.as_ptr() as *mut Atomic<SZ>;

        unsafe { (*size_ptr).store(invalid_shm_size::<SZ>(), Ordering::SeqCst) }
    }
}

/// A piece of shared memory.
///
/// Layout when `SZ` is a numeric type:
///
/// ```text
/// |<------------ total_size ------------->|
/// |                                       |
/// |<-- real_size --><-------- data ------>|
/// |                |                      |
/// |  size_of<SZ>() | size - size_of<SZ>() |
/// ```
///
/// When `SZ = EmptyShmHeader` there is no `real_size` header.
/// the full allocation is the data region.
pub struct SharedMemory<SZ: ShmHeader> {
    ptr: NonNull<u8>,
    total_size: usize,
    phantom: PhantomData<SZ>,
}

impl Deref for SharedMemory<EmptyShmHeader> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        // safe because there is no header to take care of
        unsafe { self.data() }
    }
}

impl DerefMut for SharedMemory<EmptyShmHeader> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // safe because there is no header to take care of
        unsafe { self.data_mut() }
    }
}

impl<SZ: ShmHeader> fmt::Debug for SharedMemory<SZ> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedMemory")
            .field("ptr", &self.ptr)
            .field("total_size", &self.total_size)
            .finish()
    }
}

impl<SZ: ShmHeader> Clone for SharedMemory<SZ> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            total_size: self.total_size,
            phantom: PhantomData,
        }
    }
}

impl<SZ: ShmHeader> SharedMemory<SZ> {
    /// Create a new [`SharedMemory`] view over an already-allocated region.
    ///
    /// `total_size` is the size of the **entire** allocation, including the header.
    ///
    /// # Safety
    ///
    /// `ptr` and `total_size` must describe a valid, live shared memory region.
    pub unsafe fn new(ptr: NonNull<u8>, total_size: usize) -> Result<Self> {
        if total_size < SZ::HEADER_SIZE {
            return Err(runtime!(
                "Shared memory region is too small: {} bytes",
                total_size
            ));
        }

        let mut shm = Self {
            ptr,
            total_size,
            phantom: PhantomData,
        };

        shm.mark_invalid();

        Ok(shm)
    }

    /// Total size of the allocation, including the header.
    #[must_use]
    pub fn total_len(&self) -> usize {
        self.total_size
    }

    /// Maximum size available for the data (without the header).
    #[must_use]
    pub fn max_data_len(&self) -> usize {
        self.total_size - SZ::HEADER_SIZE
    }

    /// Returns a slice of the effectively written data.
    ///
    /// # Safety
    ///
    /// For numeric `SZ`, [`Self::set_size`] must have been called after the last write through [`Self::data_mut`] with
    /// the corresponding real data size.
    ///
    /// Calling this before [`Self::data_mut`] has been called in general has an undefined behavior.
    #[must_use]
    pub unsafe fn data(&self) -> &[u8] {
        let data_size = unsafe { SZ::read_real_size(self.ptr, self.max_data_len()) }
            .expect("Invalid data size stored.");

        unsafe { slice::from_raw_parts(self.ptr.as_ptr().add(SZ::HEADER_SIZE), data_size) }
    }

    /// Returns a mutable slice covering the full writable region (excluding the header).
    ///
    /// # Safety
    ///
    /// The caller must call [`Self::set_size`] (or [`Self::mark_invalid`]) before any subsequent
    /// call to [`Self::data`].
    ///
    /// For [`EmptyShmHeader`], this is always safe to use.
    pub unsafe fn data_mut(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(
                self.ptr.as_ptr().add(SZ::HEADER_SIZE),
                self.total_size - SZ::HEADER_SIZE,
            )
        }
    }

    /// Records how many bytes of data were written.
    ///
    /// `size` is the number of data bytes, **not** including the header.
    ///
    /// # Safety
    ///
    /// Must be called after every write through [`Self::data_mut`].
    pub unsafe fn set_size(&mut self, size: usize) {
        unsafe { SZ::write_real_size(self.ptr, size) }
    }

    /// Returns the effective data size, or `None` if the region is marked invalid.
    ///
    /// For [`EmptyShmHeader`], always returns `Some(total_size)`.
    #[must_use]
    pub fn get_size(&self) -> Option<usize> {
        unsafe { SZ::read_real_size(self.ptr, self.total_size - SZ::HEADER_SIZE) }
    }

    /// Returns `true` if the region is marked invalid.
    ///
    /// For [`EmptyShmHeader`], always returns `Some(total_size)`.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.get_size().is_none()
    }

    /// Marks the region as invalid so the next receiver knows no data is ready.
    ///
    /// For [`EmptyShmHeader`], this does nothing.
    pub fn mark_invalid(&mut self) {
        unsafe { SZ::invalidate(self.ptr) }
    }

    /// Write bytes to shared memory
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        let shm_data = unsafe { self.data_mut() };

        if data.len() > shm_data.len() {
            return Err(runtime!(
                "Shm has at most {} bytes, but data to write is {} bytes long.",
                shm_data.len(),
                data.len()
            ));
        }

        unsafe {
            shm_data[..data.len()].copy_from_slice(data);
            self.set_size(data.len());
        }

        Ok(())
    }
}
