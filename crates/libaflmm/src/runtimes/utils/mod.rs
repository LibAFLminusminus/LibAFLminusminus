//! Os-specific utilities to manage some [`Runtime`](crate::runtimes::Runtime)s.
use core::{
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::NonNull,
};

#[cfg(unix)]
pub mod unix;
#[cfg(unix)]
pub use unix::{OsTerminationHandler, OsTerminationParams};

#[cfg(windows)]
pub mod windows;

pub mod termination;
pub use termination::{IntoTerminationHandlerData, TerminationHandler, TerminationHandlerData};

/// A pinned pointer wrapper type.
/// Pinning is ensure because all constructors require a pinned pointer.
#[derive(Debug)]
pub struct PinnedPtr<T> {
    ptr: NonNull<T>,
}

impl<T> Deref for PinnedPtr<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for PinnedPtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> PinnedPtr<T> {
    /// Transform (conceptually) a `Pin<&mut T>` into a `Pin<*mut T>`.
    #[must_use]
    pub fn from_pin(ptr: Pin<&mut T>) -> Self {
        let ptr = NonNull::from(unsafe { Pin::into_inner_unchecked(ptr) });
        Self { ptr }
    }

    /// Get the pinned pointer as a raw pointer
    #[must_use]
    pub fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

unsafe impl<T: Send> Send for PinnedPtr<T> {}
unsafe impl<T: Sync> Sync for PinnedPtr<T> {}
