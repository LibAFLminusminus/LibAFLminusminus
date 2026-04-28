//! Observers for `stdout` and `stderr`
//!
//! The [`StdOutObserver`] and [`StdErrObserver`] observers look at the stdout of a program
//! The executor must explicitly support these observers.
#![cfg_attr(
    unix,
    doc = r"For example, they are supported on the [`crate::executors::CommandExecutor`] and [`crate::executors::ForkserverExecutor`]."
)]

use alloc::{borrow::Cow, string::ToString, vec::Vec};
use core::marker::PhantomData;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use libafl_bolts::Named;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{DependencyResolver, Error, Registrator, observers::Observer};

/// An observer that captures stdout of a target.
/// Only works for supported executors.

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputObserver<T> {
    /// The name of the observer.
    pub name: Cow<'static, str>,
    /// The captured stdout/stderr data during last execution.
    pub output: Option<Vec<u8>>,
    #[serde(skip_serializing, deserialize_with = "new_file::<_, T>")]
    /// File backend of the memory to capture output, if [`None`] we use portable piped output
    pub file: Option<File>,
    #[serde(skip)]
    /// Phantom data to hold the stream type
    phantom: PhantomData<T>,
}

/// Blanket implementation for a [`std::fs::File`]. Fortunately the contents of the file
/// is transient and thus we can safely create a new one on deserialization (and skip it)
/// when doing serialization
fn new_file<'de, D, T>(_d: D) -> Result<Option<File>, D::Error>
where
    D: Deserializer<'de>,
{
    OutputObserver::<T>::file().map_err(|e| serde::de::Error::custom(e.to_string()))
}

/// Marker traits to mark stdout for the `OutputObserver`
#[derive(Debug, Clone)]
pub struct StdOutMarker;

/// Marker traits to mark stderr for the `OutputObserver`
#[derive(Debug, Clone)]
pub struct StdErrMarker;

impl<T> OutputObserver<T> {
    // This is the best we can do on macOS because
    // - macos doesn't have memfd_create
    // - fd returned from shm_open can't be written (https://stackoverflow.com/questions/73752631/cant-write-to-fd-from-shm-open-on-macos)
    // - there is even no native tmpfs implementation!
    // therefore we create a file and immediately remove it to get a writtable fd.
    //
    // In most cases, capturing stdout/stderr every loop is very slow and mostly for debugging purpose and thus this should be acceptable.
    #[cfg(target_os = "macos")]
    fn file() -> Result<Option<File>, Error> {
        let fp = File::create_new("fsrvmemfd")?;
        nix::unistd::unlink("fsrvmemfd")?;
        Ok(Some(fp))
    }

    /// Cool, we can have [`MemfdShMemProvider`] to create a memfd.
    #[cfg(target_os = "linux")]
    fn file() -> Result<Option<File>, Error> {
        Ok(Some(
            libafl_bolts::shmem::unix_shmem::memfd::MemfdShMemProvider::new_file()?,
        ))
    }

    /// This will use standard but portable pipe mechanism to capture outputs
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub fn file() -> Result<Option<File>, Error> {
        Ok(None)
    }

    /// Create a new [`OutputObserver`] with the given name. This will use the memory fd backend
    /// on Linux and macOS, which is compatible with forkserver.
    pub fn new(name: Cow<'static, str>) -> Result<Self, Error> {
        Ok(Self {
            name,
            output: None,
            file: Self::file()?,
            phantom: PhantomData,
        })
    }

    /// Create a new `OutputObserver` with the given name. This use portable piped backend, which
    /// only works with [`std::process::Command`].
    pub fn new_piped(name: Cow<'static, str>) -> Result<Self, Error> {
        Ok(Self {
            name,
            output: None,
            file: None,
            phantom: PhantomData,
        })
    }

    /// Create a new `OutputObserver` with given name and file.
    /// Useful for targets like nyx which writes to the same file again and again.
    #[must_use]
    pub fn new_file(name: Cow<'static, str>, file: File) -> Self {
        Self {
            name,
            output: None,
            file: Some(file),
            phantom: PhantomData,
        }
    }

    /// React to new stream data
    pub fn observe(&mut self, data: Vec<u8>) {
        self.output = Some(data);
    }

    #[must_use]
    /// Return the raw fd, if any
    pub fn as_raw_fd(&self) -> Option<i32> {
        #[cfg(target_family = "unix")]
        return self.file.as_ref().map(std::os::fd::AsRawFd::as_raw_fd);
        #[cfg(not(target_family = "unix"))]
        return None;
    }
}

impl<T> Named for OutputObserver<T> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<T> DependencyResolver for OutputObserver<T> {}

impl<S, T> Observer<S> for OutputObserver<T>
where
    T: 'static,
{
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        if let Some(file) = self.file.as_mut() {
            file.seek(SeekFrom::Start(0))?;
        }
        self.output = None;
        Ok(())
    }

    fn post_exec(
        &mut self,
        _state: &mut S,
        _exit_kind: &crate::executors::ExitKind,
    ) -> Result<(), Error> {
        if let Some(file) = self.file.as_mut()
            && self.output.is_none()
        {
            let pos = file.stream_position()?;

            file.seek(SeekFrom::Start(0))?;

            let mut buf = vec![0; pos as usize];
            file.read_exact(&mut buf)?;

            self.observe(buf);
        }
        Ok(())
    }
}

impl<T> AsRef<Self> for OutputObserver<T> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T> AsMut<Self> for OutputObserver<T> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

/// An observer that captures stdout of a target.
pub type StdOutObserver = OutputObserver<StdOutMarker>;
/// An observer that captures stderr of a target.
pub type StdErrObserver = OutputObserver<StdErrMarker>;
