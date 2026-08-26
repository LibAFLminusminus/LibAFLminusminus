//! The windows specific code for standard timers.

use crate::{
    terminations::windows::{ExceptionCode, raise_exception},
    timers::Timer,
};
use core::{ffi::c_void, ptr, time::Duration};
use libaflmm_core::Result;
use windows::{
    Win32::{
        Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, FILETIME, HANDLE, STATUS_TIMEOUT},
        System::{
            Diagnostics::Debug::{
                CONTEXT, CONTEXT_FULL_AMD64, CONTEXT_FULL_ARM64, CONTEXT_FULL_X86,
                EXCEPTION_POINTERS, EXCEPTION_RECORD, GetThreadContext,
            },
            Threading::{
                CloseThreadpoolTimer, CreateThreadpoolTimer, GetCurrentProcess, GetCurrentThread,
                PTP_CALLBACK_INSTANCE, PTP_TIMER, ResumeThread, SetThreadpoolTimer, SuspendThread,
                WaitForThreadpoolTimerCallbacks,
            },
        },
    },
    core::Owned,
};

/// Convert a [`Duration`] into a [`FILETIME`]
fn duration_to_filetime(duration: Duration) -> Result<FILETIME> {
    let due_time = (-i64::try_from(duration.as_nanos() / 100)?).cast_unsigned();

    Ok(FILETIME {
        dwLowDateTime: due_time as u32,
        dwHighDateTime: (due_time >> 32) as u32,
    })
}

/// The aligned version of [`CONTEXT`].
/// See <https://stackoverflow.com/questions/4696543/getthreadcontext-fails-after-a-successful-suspendthread-in-windows-7>
#[derive(Default)]
#[repr(align(16))]
struct AlignedContext {
    ctx: CONTEXT,
}

impl AlignedContext {
    fn new() -> Self {
        let mut context = Self::default();

        if cfg!(target_arch = "x86") {
            context.ctx.ContextFlags = CONTEXT_FULL_X86;
        } else if cfg!(any(target_arch = "x86_64", target_arch = "arm64ec")) {
            context.ctx.ContextFlags = CONTEXT_FULL_AMD64;
        } else if cfg!(target_arch = "aarch64") {
            context.ctx.ContextFlags = CONTEXT_FULL_ARM64;
        }

        context
    }

    fn instruction_pointer(&self) -> *mut c_void {
        #[cfg(target_arch = "x86")]
        let instruction_pointer = self.ctx.Eip as usize;
        #[cfg(any(target_arch = "x86_64", target_arch = "arm64ec"))]
        let instruction_pointer = self.ctx.Rip as usize;
        #[cfg(target_arch = "aarch64")]
        let instruction_pointer = self.ctx.Pc as usize;

        instruction_pointer as *mut c_void
    }
}

// get the context of the timed out thread, then raise the exception
unsafe extern "system" fn timeout_callback(
    _instance: PTP_CALLBACK_INSTANCE,
    target_thread: *mut c_void,
    _timer: PTP_TIMER,
) {
    let target_thread = HANDLE(target_thread);

    let mut context = AlignedContext::new();

    unsafe { SuspendThread(target_thread) };
    let context_result = unsafe { GetThreadContext(target_thread, &raw mut context.ctx) };
    unsafe { ResumeThread(target_thread) };

    if let Err(err) = context_result {
        log::error!("error while getting context: {err:?}");
    }

    let mut exception_record = EXCEPTION_RECORD {
        ExceptionCode: STATUS_TIMEOUT,
        ExceptionAddress: context.instruction_pointer(),
        ..Default::default()
    };

    let mut exception_pointers = EXCEPTION_POINTERS {
        ExceptionRecord: &raw mut exception_record,
        ContextRecord: &raw mut context.ctx,
    };

    unsafe { raise_exception(ExceptionCode::Timeout, &raw mut exception_pointers) };

    panic!("timeout cannot resume on windows");
}

/// Owned Win32 threadpool timer.
/// Check out <https://learn.microsoft.com/en-us/windows/win32/api/threadpoolapiset/>
#[derive(Debug)]
struct ThreadpoolTimer {
    timer: PTP_TIMER,
    _target_thread: Owned<HANDLE>,
}

impl ThreadpoolTimer {
    fn new() -> Result<Self> {
        let target_thread = unsafe {
            let mut target_thread = HANDLE::default();

            DuplicateHandle(
                GetCurrentProcess(),
                GetCurrentThread(),
                GetCurrentProcess(),
                &raw mut target_thread,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )?;

            Owned::new(target_thread)
        };

        let timer = unsafe {
            CreateThreadpoolTimer(Some(timeout_callback), Some((*target_thread).0), None)
        }?;

        Ok(Self {
            timer,
            _target_thread: target_thread,
        })
    }

    fn set(&self, expiration: Option<&FILETIME>) {
        unsafe { SetThreadpoolTimer(self.timer, expiration.map(ptr::from_ref), 0, None) };
    }

    fn wait_callbacks(&self) {
        unsafe { WaitForThreadpoolTimerCallbacks(self.timer, true) };
    }
}

impl Drop for ThreadpoolTimer {
    fn drop(&mut self) {
        self.set(None);
        self.wait_callbacks();

        unsafe { CloseThreadpoolTimer(self.timer) };
    }
}

/// A standard os-specific timer
#[derive(Debug, Default)]
pub struct StdTimer {
    timer: Option<(ThreadpoolTimer, FILETIME)>,
    disable: Option<FILETIME>,
}

impl Clone for StdTimer {
    fn clone(&self) -> Self {
        Self {
            timer: None,
            disable: self.disable,
        }
    }
}

impl StdTimer {
    /// Create an [`StdTimer`]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Timer for StdTimer {
    /// On windows, it must be called from the thread that will run the target.
    fn create_timer(&mut self, timeout: Duration) -> Result<()> {
        if self.timer.is_some() {
            self.delete_timer()?;
        }

        let expiration = duration_to_filetime(timeout)?;

        self.timer = Some((ThreadpoolTimer::new()?, expiration));

        Ok(())
    }

    unsafe fn arm_timer(&mut self) -> Result<()> {
        if let Some((timer, expiration)) = &mut self.timer {
            timer.set(Some(expiration));
        }

        Ok(())
    }

    unsafe fn disarm_timer(&mut self) -> Result<()> {
        if let Some((timer, _)) = &mut self.timer {
            timer.set(self.disable.as_ref());
            timer.wait_callbacks();
        }

        Ok(())
    }

    fn delete_timer(&mut self) -> Result<()> {
        self.timer.take();

        Ok(())
    }
}
