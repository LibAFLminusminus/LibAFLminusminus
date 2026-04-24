use core::{fmt::Debug, marker::PhantomData, pin::Pin, ptr::NonNull, time::Duration};
use std::{boxed::Box, fmt};

use libafl_bolts::TimerStruct;
use libafl_core::Error;

use crate::{
    DependencyResolver,
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

pub mod standard;
pub use standard::StdInProcessRuntime;

impl<CH, D, S, T, TH> DependencyResolver for InProcessRuntime<CH, D, S, T, TH> {}

/// Hooks the current process to set it up for in-process tasks.
/// It will change signal handlers and "pollute" the current process.
/// It is advised to combine it with the [`RestartingRuntime`], responsible
/// for forking and and state preservation.
///
/// InProcessRuntime runs a task that does NOT return.
/// To exit, simply exit the process.
/// There are special exit codes used to convey what caused the exit.
/// TODO: document these exit code
pub struct InProcessRuntime<CH, D, S, T, TH> {
    task: T,
    termination_handler: Pin<Box<OsTerminationHandler<CH, D, TH>>>,
    timer: Option<TimerStruct>,
    phantom: PhantomData<S>,
}

impl<CH, D, S, T, TH> Debug for InProcessRuntime<CH, D, S, T, TH>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InProcessRuntime")
            .field("task", &self.task)
            .finish()
    }
}

impl<CH, D, S, T, TH> Clone for InProcessRuntime<CH, D, S, T, TH>
where
    CH: Clone,
    D: Clone,
    T: Clone,
    TH: Clone,
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

impl<CH, D, S, T, TH> InProcessRuntime<CH, D, S, T, TH>
where
    CH: FnMut(&mut D, &OsTerminationParams) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    TH: FnMut(&mut D, &OsTerminationParams) -> Result<(), Error> + Send + Sync + Unpin + 'static,
{
    pub fn new(task: T, crash_handler: CH, signal_data: D, timeout_handler: TH) -> Self {
        let signal_handler = TerminationHandler::new(crash_handler, signal_data, timeout_handler);

        InProcessRuntime {
            task,
            termination_handler: Box::pin(OsTerminationHandler::new(signal_handler)),
            timer: None,
            phantom: PhantomData,
        }
    }
}

impl<CT, CH, D, S, T, TH> Runtime<CT, S> for InProcessRuntime<CH, D, S, T, TH>
where
    CH: FnMut(&mut D, &OsTerminationParams) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    T: FnMut(&mut RuntimeHandle<CT, S>, &mut S) -> Result<(), Error>,
    TH: FnMut(&mut D, &OsTerminationParams) -> Result<(), Error> + Send + Sync + Unpin + 'static,
{
    unsafe fn run_impl(
        &mut self,
        mut state: S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) -> Result<(), Error> {
        // os-specific termination handler init
        self.termination_handler.init()?;

        // set the runtime handler pointer to the termination data
        rt_handle.set_termination_handler(self.termination_handler.inner_mut().data_mut());

        (self.task)(rt_handle, &mut state)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        let timer = TimerStruct::new(timeout);
        self.timer = Some(timer);

        Ok(())
    }

    fn arm_timeout(&mut self) -> Result<(), Error> {
        if let Some(timer) = &mut self.timer {
            timer.set_timer();
        }

        Ok(())
    }

    fn disarm_timeout(&mut self) -> Result<(), Error> {
        if let Some(timer) = &mut self.timer {
            timer.unset_timer();
        }

        Ok(())
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        let mut timer = self.timer.take().expect("Could not get timer");

        timer.unset_timer();

        Ok(())
    }
}
