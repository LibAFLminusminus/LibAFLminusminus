use libafl_core::Error;

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

pub struct StdInProcessRuntime<S, T>(InnerRuntime<S, T>);

impl<S, T> StdInProcessRuntime<S, T> {
    pub fn new(state: S, task: T) -> Self {
        Self(InProcessRuntime::new(
            state,
            task,
            std_inprocess_crash,
            TerminationHandlerData::new(),
            std_inprocess_timeout,
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

impl<CT, S, T> Runtime<CT, S> for StdInProcessRuntime<S, T>
where
    T: FnMut(&mut RuntimeHandle<CT, S>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<CT, S>) -> Result<(), Error> {
        self.0.run_impl(rt_handle)
    }
}

fn std_inprocess_crash(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<(), Error> {
    data.handle_crash(signal_params);
    Ok(())
}

fn std_inprocess_timeout(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<(), Error> {
    data.handle_timeout(signal_params);
    Ok(())
}
