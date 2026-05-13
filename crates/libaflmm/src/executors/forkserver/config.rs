//! The command wrapper for properly setting up the forkserver.

use std::os::{
    fd::{BorrowedFd, FromRawFd, OwnedFd},
    unix::process::CommandExt,
};
/// Configure the target, `limit`, `setsid`, `pipe_stdin`, the code was borrowed from the [`Angora`](https://github.com/AngoraFuzzer/Angora) fuzzer
use std::{io, os::fd::RawFd, process::Command};

use libaflmm_bolts::{core_affinity::CoreId, os::last_error_str};
use libaflmm_core::forkserver::FORKSRV_FD_NUM;
use nix::{
    libc::RLIM_INFINITY,
    unistd::{close, dup2},
};

pub(crate) trait Config {
    /// Sets the sid
    fn setsid(&mut self) -> &mut Self;

    /// Sets a mem limit
    fn setlimit(&mut self, memlimit: u64) -> &mut Self;

    /// enables core dumps (rlimit = infinity)
    fn set_coredump(&mut self, enable: bool) -> &mut Self;

    /// Sets the AFL forkserver pipes
    ///
    /// # Safety
    /// All pipes must be valid file descriptors. They will be dup2-ed internally.
    unsafe fn setpipe(&mut self, st_write: RawFd, ctl_read: RawFd) -> &mut Self;

    /// [`dup2`] the specific `fd`, used for `stdio`
    ///
    /// # Safety
    /// The file descriptors must be valid. They will be `dup2-ed`.
    unsafe fn setdup2(&mut self, old_fd: RawFd, new_fd: RawFd) -> &mut Self;

    /// Bind children to a single core
    fn bind(&mut self, core: CoreId) -> &mut Self;
}

impl Config for Command {
    fn setsid(&mut self) -> &mut Self {
        let func = move || {
            // # Safety
            // raw libc call without any parameters
            unsafe {
                if libc::setsid() == -1 {
                    log::warn!("Failed to set sid. Error: {:?}", last_error_str());
                }
            };
            Ok(())
        };
        unsafe { self.pre_exec(func) }
    }

    /// # Safety
    /// All pipes must be valid file descriptors. They will be dup2-ed internally.
    unsafe fn setpipe(&mut self, st_write: RawFd, ctl_read: RawFd) -> &mut Self {
        // # Safety
        // If this was called with correct parameters, we're good.
        unsafe {
            let func = move || {
                let mut channel1 = OwnedFd::from_raw_fd(FORKSRV_FD_NUM);
                let mut channel2 = OwnedFd::from_raw_fd(FORKSRV_FD_NUM + 1);
                // Safety: these raw fds are valid in the child at pre_exec time (because at this point we created the pipe already)
                dup2(BorrowedFd::borrow_raw(ctl_read), &mut channel1).map_err(io::Error::from)?;
                dup2(BorrowedFd::borrow_raw(st_write), &mut channel2).map_err(io::Error::from)?;

                // i need this else drop() will be called on these guys
                core::mem::forget(channel1);
                core::mem::forget(channel2);

                close(ctl_read).map_err(io::Error::from)?;
                close(st_write).map_err(io::Error::from)?;
                Ok(())
            };
            self.pre_exec(func)
        }
    }

    // libc::rlim_t is i64 in freebsd and trivial_numeric_casts check will failed
    #[allow(trivial_numeric_casts)] // on 32 bit it does not trigger
    fn setlimit(&mut self, memlimit: u64) -> &mut Self {
        if memlimit == 0 {
            return self;
        }
        // # Safety
        // This method does not do shady pointer foo.
        // It merely call libc functions.
        let func = move || {
            let memlimit: libc::rlim_t = (memlimit as libc::rlim_t) << 20;
            let r = libc::rlimit {
                rlim_cur: memlimit,
                rlim_max: memlimit,
            };
            #[cfg(target_os = "openbsd")]
            let ret = unsafe { libc::setrlimit(libc::RLIMIT_RSS, &r) };
            #[cfg(not(target_os = "openbsd"))]
            let ret = unsafe { libc::setrlimit(libc::RLIMIT_AS, &raw const r) };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        // # Safety
        // This calls our non-shady function from above.
        unsafe { self.pre_exec(func) }
    }

    fn set_coredump(&mut self, enable: bool) -> &mut Self {
        let func = move || {
            let r0 = libc::rlimit {
                rlim_cur: if enable { RLIM_INFINITY } else { 0 },
                rlim_max: if enable { RLIM_INFINITY } else { 0 },
            };
            let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raw const r0) };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        // # Safety
        // This calls our non-shady function from above.
        unsafe { self.pre_exec(func) }
    }

    unsafe fn setdup2(&mut self, old_fd: RawFd, new_fd: RawFd) -> &mut Self {
        let func = move || {
            // # Safety
            // The fd should be valid at this point - depending on parameters.
            let ret = unsafe { libc::dup2(old_fd, new_fd) };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        // # Safety
        // This calls our non-shady function from above.
        unsafe { self.pre_exec(func) }
    }

    fn bind(&mut self, core: CoreId) -> &mut Self {
        let func = move || {
            if let Err(e) = core.set_affinity_forced() {
                return Err(io::Error::other(e));
            }

            Ok(())
        };
        // # Safety
        // This calls our non-shady function from above.
        unsafe { self.pre_exec(func) }
    }
}
