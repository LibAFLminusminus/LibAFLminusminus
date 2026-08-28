#[cfg(unix)]
pub mod unix;
#[cfg(unix)]
pub use unix::TerminationCode;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::TerminationCode;
