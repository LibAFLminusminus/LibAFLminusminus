use libafl_core::Error;

use crate::runtimes::{
    SignalHandlerData,
    inprocess::{InProcessRuntime, unix::OsSignalHandlerParams},
};

pub type StdInProcessRuntime<S, T> = InProcessRuntime<
    fn(&mut SignalHandlerData, &OsSignalHandlerParams) -> Result<(), Error>,
    SignalHandlerData,
    S,
    T,
    fn(&mut SignalHandlerData, &OsSignalHandlerParams) -> Result<(), Error>,
>;

impl<S, T> StdInProcessRuntime<S, T> {
    pub fn new(state: S, task: T) -> Self {
        InProcessRuntime::new_generic(
            state,
            task,
            std_inprocess_crash,
            SignalHandlerData::new(),
            std_inprocess_timeout,
        )
    }
}

fn std_inprocess_crash(
    data: &mut SignalHandlerData,
    signal_params: &OsSignalHandlerParams,
) -> Result<(), Error> {
    data.handle_crash(signal_params);
    Ok(())
}

fn std_inprocess_timeout(
    data: &mut SignalHandlerData,
    signal_params: &OsSignalHandlerParams,
) -> Result<(), Error> {
    data.handle_timeout(signal_params);
    Ok(())
}
