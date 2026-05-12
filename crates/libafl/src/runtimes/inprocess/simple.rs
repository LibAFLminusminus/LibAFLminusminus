//! A simple in-process [`Runtime`].

use core::time::Duration;

use libafl_bolts::timers::Timer;
use libafl_core::Result;
use serde::Serialize;

use crate::{
    DependencyResolver,
    runtimes::{
        Runtime, RuntimeHandle, TerminationHandlerData,
        inprocess::{CrashStatus, InProcessRuntime, TimeoutStatus},
        utils::OsTerminationParams,
    },
};

type InnerRuntime<S, T, TM> = InProcessRuntime<
    fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<CrashStatus>,
    TerminationHandlerData,
    S,
    T,
    fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<TimeoutStatus>,
    TM,
>;

/// A simple in-process [`Runtime`].
#[derive(Debug)]
pub struct SimpleInProcessRuntime<S, T, TM>(InnerRuntime<S, T, TM>);

impl<S, T, TM> Clone for SimpleInProcessRuntime<S, T, TM>
where
    T: Clone,
    TM: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S, T, TM> SimpleInProcessRuntime<S, T, TM>
where
    S: Serialize,
{
    /// Create a new simple in-process [`Runtime`].
    pub fn new(task: T, timer: TM) -> Self {
        Self(InProcessRuntime::new(
            task,
            std_inprocess_crash::<S>,
            TerminationHandlerData::new(),
            std_inprocess_timeout::<S>,
            timer,
        ))
    }
}

impl<S, T, TM> DependencyResolver for SimpleInProcessRuntime<S, T, TM> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        self.0.register(registrator)
    }

    fn register_with_ty(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        registrator.register_ty::<InnerRuntime<S, T, TM>>();

        self.register(registrator)
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<()> {
        self.0.check(checker)
    }
}

impl<S, W, T, TM> Runtime<S, W> for SimpleInProcessRuntime<S, T, TM>
where
    T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()>,
    TM: Timer,
{
    unsafe fn run_impl(self, state: S, rt_handle: &mut RuntimeHandle<S, W>) -> Result<()> {
        unsafe { self.0.run_impl(state, rt_handle) }
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.0.set_timeout(timeout)
    }

    fn arm_timeout(&mut self) -> Result<()> {
        self.0.arm_timeout()
    }

    fn disarm_timeout(&mut self) -> Result<()> {
        self.0.disarm_timeout()
    }

    fn unset_timeout(&mut self) -> Result<()> {
        self.0.unset_timeout()
    }
}

fn std_inprocess_crash<S: Serialize>(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<CrashStatus> {
    let res = data.handle_crash(signal_params)?;

    if let Some(saver) = unsafe { data.saver::<S>() } {
        unsafe {
            saver.send(data.state())?;
        }
    }

    Ok(res)
}

fn std_inprocess_timeout<S: Serialize>(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<TimeoutStatus> {
    let res = data.handle_timeout(signal_params)?;

    if let Some(saver) = unsafe { data.saver::<S>() } {
        unsafe {
            saver.send(data.state())?;
        }
    }

    Ok(res)
}
