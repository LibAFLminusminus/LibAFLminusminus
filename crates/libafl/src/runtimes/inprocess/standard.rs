use libafl_core::Error;

use crate::runtimes::{
    RuntimeHandle, SignalHandlerData,
    inprocess::{InProcessRuntime, InProcessSignalHandler, OsSignalHandler},
};

pub type StdInProcessSignalHandler = fn(&mut SignalHandlerData) -> Result<(), Error>;

pub type StdInProcessRuntime<CT, I, O, OF, S, T> = InProcessRuntime<
    StdInProcessSignalHandler,
    SignalHandlerData,
    S,
    T,
    StdInProcessSignalHandler,
>;

impl<CT, I, O, OF, S, T> StdInProcessRuntime<CT, I, O, OF, S, T>
where
    CT: 'static,
    I: 'static,
    O: 'static,
    OF: 'static,
    S: 'static,
{
    pub fn new(state: S, task: T) -> Self {
        InProcessRuntime::new_generic(
            state,
            task,
            std_inprocess_crash::<CT, I, O, OF, S>,
            SignalHandlerData::new(),
            std_inprocess_timeout::<CT, I, O, OF, S>,
        )
    }
}

fn std_inprocess_crash<CT, I, O, OF, S>(data: &mut SignalHandlerData) -> Result<(), Error> {
    data.handle_crash();
    Ok(())
}

fn std_inprocess_timeout<CT, I, O, OF, S>(data: &mut SignalHandlerData) -> Result<(), Error> {
    data.handle_timeout();
    Ok(())
}
