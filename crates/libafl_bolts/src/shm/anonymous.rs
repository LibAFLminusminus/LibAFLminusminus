//! A simple POSIX Shared Memory model

use crate::shm::SharedMemory;
use core::marker::PhantomData;
use libafl_core::{Result, non_zero, runtime, serialize};
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use num_traits::{Bounded, NumCast};
use serde::{Deserialize, Serialize};
use wide::bytemuck::NoUninit;

/// A helper builder
///
/// [`AnonShmSender`] and [`AnonShmReceiver`] should be built from here.
#[derive(Debug)]
pub struct AnonShmBuilder;

/// A shared memory sender
///
/// It sends some value on the shared memory,
/// ready to be consumed by the [`AnonShmReceiver`]
#[derive(Debug)]
pub struct AnonShmSender<SZ: NoUninit, T> {
    shm: SharedMemory<SZ>,
    phantom: PhantomData<T>,
}

/// A shared memory receiver
///
/// It receives memory sent by the [`AnonShmSender`].
#[derive(Debug)]
pub struct AnonShmReceiver<SZ: NoUninit, T> {
    shm: SharedMemory<SZ>,
    phantom: PhantomData<T>,
}

impl AnonShmBuilder {
    /// Build a pair of shared data sender / receiver.
    /// The sender writes the data to share, and the receiver handles it.
    /// Once data is received, the shared memory is marked as invalid until the sender sends something new.
    pub fn build<SZ, T>(max_data_size: SZ) -> Result<(AnonShmSender<SZ, T>, AnonShmReceiver<SZ, T>)>
    where
        SZ: NoUninit + NumCast + Bounded + PartialEq,
    {
        let max_data_size_usize: usize = NumCast::from(max_data_size).unwrap();
        let shm_size_usize: usize = max_data_size_usize + size_of::<SZ>();
        let shm_size: SZ = NumCast::from(shm_size_usize).unwrap();

        let nonzero_shm_size = non_zero!(shm_size_usize);

        let shared_memory = unsafe {
            mmap_anonymous(
                None,
                nonzero_shm_size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
            )
            .expect("Could not allocate shared memory for the shm sender")
        };

        unsafe {
            let shm = SharedMemory::new(shared_memory.cast(), shm_size)?;

            Ok((AnonShmSender::new(shm.clone()), AnonShmReceiver::new(shm)))
        }
    }
}

impl<SZ: NoUninit, T> AnonShmSender<SZ, T> {
    unsafe fn new(shm: SharedMemory<SZ>) -> Self {
        Self {
            shm,
            phantom: PhantomData,
        }
    }
}

impl<SZ: NoUninit, T> AnonShmReceiver<SZ, T> {
    unsafe fn new(shm: SharedMemory<SZ>) -> Self {
        Self {
            shm,
            phantom: PhantomData,
        }
    }
}

impl<SZ, T> AnonShmSender<SZ, T>
where
    SZ: NoUninit + NumCast + Bounded + PartialEq,
    T: Serialize,
{
    /// Send some data on the shared memory
    ///
    /// It should be received by the paired [`AnonShmReceiver`].
    pub fn send(&mut self, data: &T) -> Result<()> {
        let data_shm = unsafe { self.shm.data_mut() };

        let used_len_usize = match postcard::to_slice(data, data_shm) {
            Ok(used_slice) => used_slice.len(),
            Err(e) => {
                return Err(serialize!("Error while serializing data: {e}"));
            }
        };

        let used_len: SZ = NumCast::from(used_len_usize).unwrap();
        unsafe {
            self.shm.set_size(used_len);
        }

        Ok(())
    }
}

impl<SZ, T> AnonShmReceiver<SZ, T>
where
    SZ: NoUninit + NumCast + Bounded + PartialEq,
    for<'de> T: Deserialize<'de>,
{
    /// # Safety
    ///
    /// AnonShmSender::send must be called BEFORE calling this function
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
