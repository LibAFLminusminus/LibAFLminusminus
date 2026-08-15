//! os-specific timers.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::StdTimer;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::StdTimer;
