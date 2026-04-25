use core::time::Duration;

use libafl_core::Error;
use serde::Serialize;

use crate::{
    DependencyResolver,
    runtimes::{
        Runtime, RuntimeHandle, TerminationHandlerData, inprocess::InProcessRuntime,
        utils::OsTerminationParams,
    },
};

type InnerRuntime<S, T> = InProcessRuntime<
    fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<(), Error>,
    TerminationHandlerData,
    S,
    T,
    fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<(), Error>,
>;

#[derive(Debug)]
pub struct StdInProcessRuntime<S, T>(InnerRuntime<S, T>);

impl<S, T> Clone for StdInProcessRuntime<S, T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S, T> StdInProcessRuntime<S, T>
where
    S: Serialize,
{
    pub fn new(task: T) -> Self {
        Self(InProcessRuntime::new(
            task,
            std_inprocess_crash::<S>,
            TerminationHandlerData::new(),
            std_inprocess_timeout::<S>,
        ))
    }
}

impl<S, T> DependencyResolver for StdInProcessRuntime<S, T> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.0.register(registrator)
    }

    fn register_with_ty(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        registrator.register_ty::<Self>();
        registrator.register_ty::<InnerRuntime<S, T>>();

        self.register(registrator)
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        self.0.check(checker)
    }
}

impl<S, W, T> Runtime<S, W> for StdInProcessRuntime<S, T>
where
    T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(
        &mut self,
        state: S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<(), Error> {
        self.0.run_impl(state, rt_handle)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.0.set_timeout(timeout)
    }

    fn arm_timeout(&mut self) -> Result<(), Error> {
        self.0.arm_timeout()
    }

    fn disarm_timeout(&mut self) -> Result<(), Error> {
        self.0.disarm_timeout()
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        self.0.unset_timeout()
    }
}

fn std_inprocess_crash<S: Serialize>(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<(), Error> {
    if data.handle_crash(signal_params)
        && let Some(saver) = unsafe { data.saver::<S>() }
    {
        unsafe {
            saver.send(data.state());
        }
    }

    Ok(())
}

fn std_inprocess_timeout<S: Serialize>(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<(), Error> {
    if data.handle_timeout(signal_params)
        && let Some(saver) = unsafe { data.saver::<S>() }
    {
        unsafe {
            saver.send(data.state());
        }
    }

    Ok(())
}
