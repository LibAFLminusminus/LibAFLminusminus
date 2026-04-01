use crate::{
    executors::hooks::timer::TimerStruct,
    runners::{Runner, RunnerDriver},
};
use core::{convert::Infallible, pin::Pin, time::Duration};
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
pub struct InProcessRunner<CH, S, T, TH> {
    state: S,
    task: T,
    signal_handler: Pin<Box<OsSignalHandler<CH, TH>>>,
    timer: Option<TimerStruct>,
}

pub struct InProcessSignalHandler<CH, TH> {
    signal_handler_depth: usize,
    signal_handler_max_depth: usize,
    crash_handler: CH,
    timeout_handler: TH,
    in_target: bool,
}

unsafe impl<CH, TH> Send for InProcessSignalHandler<CH, TH>
where
    CH: Send,
    TH: Send,
{
}

unsafe impl<CH, TH> Sync for InProcessSignalHandler<CH, TH>
where
    CH: Sync,
    TH: Sync,
{
}

impl<CH, S, T, TH> InProcessRunner<CH, S, T, TH> {
    pub fn new(state: S, task: T, crash_handler: CH, timeout_handler: TH) -> Self {
        let signal_handler = InProcessSignalHandler::new(crash_handler, timeout_handler);

        InProcessRunner {
            state,
            task,
            signal_handler: Box::pin(OsSignalHandler::new(signal_handler)),
            timer: None,
        }
    }
}

impl<CH, TH> InProcessSignalHandler<CH, TH> {
    pub fn new(crash_handler: CH, timeout_handler: TH) -> Self {
        Self {
            crash_handler,
            timeout_handler,
            signal_handler_depth: 0,
            signal_handler_max_depth: 3,
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
        self.timeout_handler()
    }

    pub fn handle_crash(&mut self) {
        self.crash_handler()
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

impl<CH, S, T, TH> Runner<S> for InProcessRunner<CH, S, T, TH>
where
    T: for<'a> FnOnce(RunnerDriver<'a, Self, S>, &mut S) -> Result<Infallible, Error>,
    CH: FnMut(&mut S) -> Result<(), Error>,
    TH: FnMut(&mut S) -> Result<(), Error>,
{
    // TODO: handle signals
    fn run_task<'a>(
        &'a mut self,
        driver: RunnerDriver<'a, Self, S>,
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
