//! Operating System specific abstractions

use crate::Error;
#[cfg(unix)]
use libc::pid_t;
use std::{env, process::Command};

/// Child Process Handle
#[cfg(unix)]
#[derive(Debug)]
pub struct ChildHandle {
    /// The process id
    pub pid: pid_t,
}

/// Returns the last OS error (errno).
#[must_use]
pub fn last_os_error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap()
}

/// The `ForkResult` (result of a fork)
#[cfg(unix)]
#[derive(Debug)]
pub enum ForkResult {
    /// The fork finished, we are the parent process.
    /// The child has the handle `ChildHandle`.
    Parent(ChildHandle),
    /// The fork finished, we are the child process.
    Child,
}

/// Unix has forks.
/// # Safety
/// A Normal fork. Runs on in two processes. Should be memory safe in general.
#[cfg(unix)]
pub unsafe fn fork() -> Result<ForkResult, Error> {
    unsafe {
        match libc::fork() {
            pid if pid > 0 => Ok(ForkResult::Parent(ChildHandle { pid })),
            pid if pid < 0 => {
                // Getting errno from rust is hard, we'll just let the libc print to stderr for now.
                // In any case, this should usually not happen.
                {
                    let err_str = CString::new("Fork failed").unwrap();
                    libc::perror(err_str.as_ptr());
                }
                Err(Error::unknown(format!("Fork failed ({pid})")))
            }
            _ => Ok(ForkResult::Child),
        }
    }
}

/// Executes the current process from the beginning, as subprocess.
/// use `start_self.status()?` to wait for the child
pub fn startable_self() -> Result<Command, Error> {
    let mut startable = Command::new(env::current_exe()?);
    startable
        .current_dir(env::current_dir()?)
        .args(env::args().skip(1));
    Ok(startable)
}

/// "Safe" wrapper around `dup`, duplicating the given file descriptor
///
/// # Safety
/// The fd need to be a legal fd.
#[cfg(unix)]
pub unsafe fn dup(fd: RawFd) -> Result<RawFd, Error> {
    match unsafe { libc::dup(fd) } {
        -1 => Err(Error::last_os_error(format!("Error calling dup({fd})"))),
        new_fd => Ok(new_fd),
    }
}
