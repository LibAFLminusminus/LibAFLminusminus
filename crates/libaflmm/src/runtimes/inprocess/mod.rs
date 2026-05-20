//! In-process [`Runtime`]s.

use alloc::boxed::Box;
use core::{
    fmt::{self, Debug},
    marker::PhantomData,
    pin::Pin,
    time::Duration,
};

use libaflmm_bolts::timers::Timer;
use libaflmm_core::Result;

use crate::{
    common::DependencyResolver,
    runtimes::{
        Runtime, RuntimeHandle,
        utils::{
            IntoTerminationHandlerData, OsTerminationHandler, OsTerminationParams,
            TerminationHandler,
        },
    },
};

#[cfg(test)]
mod tests;

pub mod simple;
pub use simple::SimpleInProcessRuntime;

/// The status of a crash.
#[derive(Debug, Default)]
pub enum CrashStatus {
    /// The crash is caused by the fuzzer, it's a `LibAFLmm` bug
    FuzzerCrash,
    /// The crash is caused by the target, it's a target bug
    #[default]
    TargetCrash,
}

/// The status of a timeout
#[derive(Debug, Default)]
pub enum TimeoutStatus {
    /// Resume on timeout
    Resume,
    /// Exit with timeout error code on timeout
    #[default]
    Exit,
}

impl<CH, D, S, T, TH, TM> DependencyResolver for InProcessRuntime<CH, D, S, T, TH, TM> {}

/// Hooks the current process to set it up for in-process tasks.
/// It will change signal handlers and "pollute" the current process.
/// It is advised to combine it with the [`crate::runtimes::RestartingRuntime`], responsible
/// for forking and and state preservation.
///
/// [`InProcessRuntime`] runs a task that does NOT return.
/// To exit, simply exit the process.
/// There are special exit codes used to convey what caused the exit.
/// TODO: document these exit code
pub struct InProcessRuntime<CH, D, S, T, TH, TM> {
    task: T,
    termination_handler: Pin<Box<OsTerminationHandler<CH, D, TH>>>,
    timer: TM,
    phantom: PhantomData<S>,
}

impl<CH, D, S, T, TH, TM> Debug for InProcessRuntime<CH, D, S, T, TH, TM>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InProcessRuntime")
            .field("task", &self.task)
            .finish_non_exhaustive()
    }
}

impl<CH, D, S, T, TH, TM> Clone for InProcessRuntime<CH, D, S, T, TH, TM>
where
    CH: Clone,
    D: Clone,
    T: Clone,
    TH: Clone,
    TM: Clone,
{
    fn clone(&self) -> Self {
        Self {
            task: self.task.clone(),
            termination_handler: self.termination_handler.clone(),
            timer: self.timer.clone(),
            phantom: PhantomData,
        }
    }
}

impl<CH, D, S, T, TH, TM> InProcessRuntime<CH, D, S, T, TH, TM>
where
    CH: FnMut(&mut D, &OsTerminationParams) -> Result<CrashStatus> + Send + Sync + Unpin + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    TH: FnMut(&mut D, &OsTerminationParams) -> Result<TimeoutStatus>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    /// Create a new [`InProcessRuntime`].
    pub fn new(task: T, crash_handler: CH, signal_data: D, timeout_handler: TH, timer: TM) -> Self {
        let signal_handler = TerminationHandler::new(crash_handler, signal_data, timeout_handler);

        InProcessRuntime {
            task,
            termination_handler: Box::pin(OsTerminationHandler::new(signal_handler)),
            timer,
            phantom: PhantomData,
        }
    }
}

impl<CH, D, S, T, TH, TM, W> Runtime<S, W> for InProcessRuntime<CH, D, S, T, TH, TM>
where
    CH: FnMut(&mut D, &OsTerminationParams) -> Result<CrashStatus> + Send + Sync + Unpin + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()>,
    TH: FnMut(&mut D, &OsTerminationParams) -> Result<TimeoutStatus>
        + Send
        + Sync
        + Unpin
        + 'static,
    TM: Timer,
{
    unsafe fn run_impl(&mut self, mut state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()> {
        // OS-specific termination handler init
        self.termination_handler.init()?;

        let termination_data: Pin<&mut D> = {
            let handler: Pin<&mut OsTerminationHandler<CH, D, TH>> =
                self.termination_handler.as_mut();

            unsafe { handler.map_unchecked_mut(|hdlr| &mut hdlr.inner.termination_data) }
        };

        // set the runtime handler pointer to the termination data
        if let Some(termination_handler_data) = D::termination_handler_data(termination_data) {
            unsafe {
                rt_handle.set_termination_handler(termination_handler_data);
            }
        }

        (self.task)(rt_handle, &mut state)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.timer.create_timer(timeout)
    }

    fn arm_timeout(&mut self) -> Result<()> {
        unsafe { self.timer.arm_timer() }
    }

    fn disarm_timeout(&mut self) -> Result<()> {
        unsafe { self.timer.disarm_timer() }
    }

    fn unset_timeout(&mut self) -> Result<()> {
        self.timer.delete_timer()
    }
}
