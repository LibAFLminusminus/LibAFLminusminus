use crate::runners::{Runner, RunnerDriver};
use core::{convert::Infallible, pin::Pin, time::Duration};
use libafl_bolts::TimerStruct;
use libafl_core::Error;
use std::boxed::Box;

pub mod unix;
pub use unix::OsSignalHandler;

/// Hooks the current process to set it up for in-process tasks.
/// It will change signal handlers and "pollute" the current process.
/// It is advised to combine it with the [`RestartingRunner`], responsible
/// for forking and and state preservation.
///
/// InProcessRunner runs a task that does NOT return.
/// To exit, simply exit the process.
/// There are special exit codes used to convey what caused the exit.
/// TODO: document these exit code
pub struct InProcessRunner<CH, D, S, T, TH> {
    state: S,
    task: T,
    signal_handler: Pin<Box<OsSignalHandler<CH, D, TH>>>,
    timer: Option<TimerStruct>,
}

pub struct InProcessSignalHandler<CH, D, TH> {
    signal_handler_depth: usize,
    signal_handler_max_depth: usize,
    crash_handler: CH,
    timeout_handler: TH,
    signal_data: D,
    in_target: bool,
    // this should hold any pointer to data needed in signal handling.
}

unsafe impl<CH, D, TH> Send for InProcessSignalHandler<CH, D, TH>
where
    CH: Send,
    D: Send,
    TH: Send,
{
}

unsafe impl<CH, D, TH> Sync for InProcessSignalHandler<CH, D, TH>
where
    CH: Sync,
    D: Sync,
    TH: Sync,
{
}

impl<CH, D, S, T, TH> InProcessRunner<CH, D, S, T, TH> {
    pub fn new(state: S, task: T, crash_handler: CH, signal_data: D, timeout_handler: TH) -> Self {
        let signal_handler =
            InProcessSignalHandler::new(crash_handler, signal_data, timeout_handler);

        InProcessRunner {
            state,
            task,
            signal_handler: Box::pin(OsSignalHandler::new(signal_handler)),
            timer: None,
        }
    }
}

impl<CH, D, TH> InProcessSignalHandler<CH, D, TH>
where
    CH: FnMut(&mut D, &mut S) -> Result<(), Error>,
    TH: FnMut(&mut D, &mut S) -> Result<(), Error>,
{
    pub fn new(crash_handler: CH, signal_data: D, timeout_handler: TH) -> Self {
        Self {
            crash_handler,
            timeout_handler,
            signal_handler_depth: 0,
            signal_handler_max_depth: 3,
            signal_data,
            in_target: false,
        }
    }

    pub fn enter(&mut self) -> bool {
        self.signal_handler_depth += 1;

        self.signal_handler_depth >= self.signal_handler_max_depth
    }

    pub fn exit(&mut self) {
        self.signal_handler_depth -= 1;
    }

    pub fn handle_timeout(&mut self) {
        (self.timeout_handler)(&mut self.signal_data)
    }

    pub fn handle_crash(&mut self) {
        (self.crash_handler)(&mut self.signal_data)
    }

    pub fn max_depth(&self) -> usize {
        self.signal_handler_max_depth
    }

    pub fn enter_target(&mut self) {
        self.in_target = true;
    }

    pub fn exit_target(&mut self) {
        self.in_target = false;
    }

    pub fn is_in_target(&self) -> bool {
        self.in_target
    }
}

impl<CH, D, S, T, TH> Runner<S> for InProcessRunner<CH, D, S, T, TH>
where
    T: FnOnce(&mut RunnerDriver<S>, &mut S) -> Result<Infallible, Error>,
    CH: FnMut(&mut S) -> Result<(), Error>,
    TH: FnMut(&mut S) -> Result<(), Error>,
{
    // TODO: handle signals
    unsafe fn run_impl(
        &mut self,
        driver: &mut RunnerDriver<S>,
        state: &mut S,
    ) -> Result<(), Error> {
        self.signal_handler.init();

        self.task(driver, state)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.timer = Some(TimerStruct::new(timeout.clone()));

        Ok(())
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        let mut timer = self.timer.take();

        timer.unset_timer();

        Ok(())
    }
}
