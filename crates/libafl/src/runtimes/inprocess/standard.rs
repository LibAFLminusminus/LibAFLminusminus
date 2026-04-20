use core::ptr::NonNull;
use std::boxed::Box;

use libafl_core::Error;

use crate::runtimes::{
    RuntimeHandle,
    inprocess::{InProcessRuntime, InProcessSignalHandler, OsSignalHandler},
};

pub type StdInProcessSignalHandler<CT, I, O, OF, S> =
    fn(&mut StdInProcessData<CT, I, O, OF, S>) -> Result<(), Error>;

pub type StdInProcessRuntime<CT, I, O, OF, S, T> = InProcessRuntime<
    StdInProcessSignalHandler<CT, I, O, OF, S>,
    StdInProcessData<CT, I, O, OF, S>,
    S,
    T,
    StdInProcessSignalHandler<CT, I, O, OF, S>,
>;

impl<CT, I, O, OF, S, T> StdInProcessRuntime<CT, I, O, OF, S, T>
where
    I: 'static,
    O: 'static,
    OF: 'static,
    S: 'static,
{
    pub fn new(state: S, task: T) -> Self {
        let data = StdInProcessData::new();

        InProcessRuntime::new_generic(
            state,
            task,
            std_inprocess_crash::<CT, I, O, OF, S>,
            data,
            std_inprocess_timeout::<CT, I, O, OF, S>,
        )
    }
}

pub struct StdInProcessData<CT, I, O, OF, S>
where
    CT: 'static,
{
    observers: Option<NonNull<O>>,
    state: Option<NonNull<S>>,
    input: Option<NonNull<I>>,
    objective_feedback: Option<NonNull<OF>>,
    rt_handle: Option<NonNull<RuntimeHandle<'static, CT, S>>>,
}

unsafe impl<CT, I, O, OF, S> Send for StdInProcessData<CT, I, O, OF, S> {}
unsafe impl<CT, I, O, OF, S> Sync for StdInProcessData<CT, I, O, OF, S> {}

impl<CT, I, O, OF, S> Default for StdInProcessData<CT, I, O, OF, S> {
    fn default() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl<CT, I, O, OF, S> StdInProcessData<CT, I, O, OF, S> {
    pub fn new() -> Self {
        Self::default()
    }
}

fn std_inprocess_timeout<CT, I, O, OF, S>(
    data: &mut StdInProcessData<CT, I, O, OF, S>,
) -> Result<(), Error> {
    eprintln!("Timeout triggered!");

    Ok(())
}

fn std_inprocess_crash<CT, I, O, OF, S>(
    data: &mut StdInProcessData<CT, I, O, OF, S>,
) -> Result<(), Error> {
    eprintln!("Crash triggered!");

    Ok(())
}
