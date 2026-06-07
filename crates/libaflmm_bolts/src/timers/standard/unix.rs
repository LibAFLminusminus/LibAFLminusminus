//! The struct `TimerStruct` will absorb all the difference in timeout implementation in various system.
use core::time::Duration;
pub(crate) const ITIMER_REAL: core::ffi::c_int = 0;

fn duration_to_itimerspec(duration: Duration) -> libc::itimerspec {
    let milli_sec = duration.as_millis();

    let it_value = libc::timespec {
        tv_sec: (milli_sec / 1000) as _,
        tv_nsec: ((milli_sec % 1000) * 1000 * 1000) as _,
    };

    let it_interval = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    libc::itimerspec {
        it_interval,
        it_value,
    }
}

#[repr(C)]
#[cfg(all(unix, not(target_os = "linux")))]
#[derive(Copy, Clone)]
pub(crate) struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[cfg(all(unix, not(target_os = "linux")))]
impl core::fmt::Debug for Timeval {
    #[expect(clippy::cast_sign_loss)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Timeval {{ tv_sec: {:?}, tv_usec: {:?} (tv: {:?}) }}",
            self.tv_sec,
            self.tv_usec,
            Duration::new(self.tv_sec as _, (self.tv_usec * 1000) as _)
        )
    }
}

#[repr(C)]
#[cfg(all(unix, not(target_os = "linux")))]
#[derive(Debug, Copy, Clone)]
pub(crate) struct Itimerval {
    pub it_interval: Timeval,
    pub it_value: Timeval,
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe extern "C" {
    pub(crate) fn setitimer(
        which: libc::c_int,
        new_value: *mut Itimerval,
        old_value: *mut Itimerval,
    ) -> libc::c_int;
}

/// The strcut about all the internals of the timer.
/// This struct absorb all platform specific differences about timer.
#[expect(missing_debug_implementations)]
pub struct TimerStruct {
    #[cfg(all(unix, not(target_os = "linux")))]
    itimerval: Itimerval,
    #[cfg(target_os = "linux")]
    pub(crate) timerid: libc::timer_t,
    #[cfg(target_os = "linux")]
    pub(crate) itimerspec: libc::itimerspec,
}

impl Clone for TimerStruct {
    fn clone(&self) -> Self {
        Self {
            #[cfg(all(unix, not(target_os = "linux")))]
            itimerval: self.itimerval.clone(),
            #[cfg(target_os = "linux")]
            timerid: null_mut(),
            #[cfg(target_os = "linux")]
            itimerspec: self.itimerspec.clone(),
        }
    }
}

impl TimerStruct {
    /// Create a `TimerStruct` with the specified timeout
    #[cfg(all(unix, not(target_os = "linux")))]
    #[must_use]
    pub fn new(exec_tmout: Duration) -> Self {
        let milli_sec = exec_tmout.as_millis();
        let it_value = Timeval {
            tv_sec: (milli_sec / 1000) as i64,
            tv_usec: (milli_sec % 1000) as i64,
        };
        let it_interval = Timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let itimerval = Itimerval {
            it_interval,
            it_value,
        };
        Self { itimerval }
    }

    #[cfg(target_os = "linux")]
    #[must_use]
    /// Create a `TimerStruct` with the specified timeout.
    ///
    /// # Safety
    ///
    /// It must be created in the process in which the fuzzer will run.
    pub unsafe fn new() -> Self {
        let timerid: libc::timer_t = null_mut();
        let itimerspec = libc::itimerspec {
            it_interval: libc::timespec {
                ..Default::default()
            },
            it_value: libc::timespec {
                ..Default::default()
            },
        };

        Self {
            itimerspec,
            timerid,
        }
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    /// Set up timer
    pub fn set_timer(&mut self) {
        #[cfg(not(miri))]
        unsafe {
            // creates a new per-process interval timer
            libc::timer_create(libc::CLOCK_MONOTONIC, null_mut(), &raw mut timerid);
        }

        // # Safety
        // Safe because the variables are all alive at this time and don't contain pointers.
        unsafe {
            setitimer(ITIMER_REAL, &raw mut self.itimerval, core::ptr::null_mut());
        }
    }

    /// Set up timer
    #[cfg(target_os = "linux")]
    pub fn set_timer(&mut self, timeout: Duration) {
        let spec = duration_to_itimerspec(timeout);
        self.itimerspec = spec;

        #[cfg(not(miri))]
        unsafe {
            // creates a new per-process interval timer
            libc::timer_create(libc::CLOCK_MONOTONIC, null_mut(), &raw mut self.timerid);
        }

        #[cfg(not(miri))]
        unsafe {
            libc::timer_settime(self.timerid, 0, &raw mut self.itimerspec, null_mut());
        }
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    /// Disable the timer
    pub fn unset_timer(&mut self) {
        // # Safety
        // No user-provided values.
        unsafe {
            let mut itimerval_zero: Itimerval = core::mem::zeroed();
            setitimer(ITIMER_REAL, &raw mut itimerval_zero, core::ptr::null_mut());
        }
    }

    /// Disable the timer
    #[cfg(target_os = "linux")]
    pub fn unset_timer(&mut self) {
        // # Safety
        // Just API calls, no user-provided inputs
        unsafe {
            let disarmed: libc::itimerspec = zeroed();
            libc::timer_settime(self.timerid, 0, &raw const disarmed, null_mut());
        }
    }
}
