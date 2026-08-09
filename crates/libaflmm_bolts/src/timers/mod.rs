//! Timers collection

use core::time::Duration;

use libaflmm_core::Result;

#[cfg(unix)]
pub mod fast;
#[cfg(unix)]
pub use fast::FastTimer;

pub mod standard;
pub use standard::StdTimer;

/// Timer interface
pub trait Timer: Clone {
    /// Set the timeout value.
    /// It must be called before using the timer.
    /// This function must be called in the process in which it will be used.
    ///
    /// No need to `delete_timer` before calling this.
    ///
    /// It should not start the timer, only create it.
    /// `timeout` is guaranteed to be the timeout used on all the next `arm_timer` call.
    fn create_timer(&mut self, timeout: Duration) -> Result<()>;

    /// Arm the timer with the `timeout` value set by `create_timer`
    ///
    /// # Safety
    ///
    /// This must never be called before calling `create_timer`
    unsafe fn arm_timer(&mut self) -> Result<()>;

    /// Disarm the timer
    ///
    /// # Safety
    ///
    /// This must never be called before calling `create_timer`
    unsafe fn disarm_timer(&mut self) -> Result<()>;

    /// Disable the timer, making it inactive.
    ///
    /// Any call to `arm_timer` / `disarm_timer` becomes invalid until another call to `set_timeout` is made.
    fn delete_timer(&mut self) -> Result<()>;
}
