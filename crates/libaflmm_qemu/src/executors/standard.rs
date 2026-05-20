//! A `QEMU`-based executor for binary-only instrumentation in `LibAFL`

use crate::Emulator;
use libaflmm::{
    Result,
    common::DependencyResolver,
    controllers::Worker,
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    runtimes::{
        OsTerminationParams, RuntimeHandle,
        inprocess::{CrashStatus, TimeoutStatus},
    },
};
use libaflmm_bolts::tuples::RefIndexable;

pub struct StdQemuExecutor<EMU, OT, PRE, POST> {
    emulator: EMU,
    pre_exec: PRE,
    post_exec: POST,
    observers: OT,
}

impl<EMU, OT, PRE, POST> StdQemuExecutor<EMU, OT, PRE, POST> {
    pub fn new(
        _state: &mut EMU::State, // only used to help the type system infer the real type of S.
        emulator: EMU,
        pre_exec: PRE,
        post_exec: POST,
        observers: OT,
    ) -> Result<Self>
    where
        EMU: Emulator,
        PRE: FnMut(&mut EMU::State, &EMU::Input, &mut EMU) -> Result<()>,
        POST: FnMut(&mut EMU::State, &EMU::Input, &mut EMU, &mut ExitKind) -> Result<()>,
    {
        Ok(Self {
            emulator,
            pre_exec,
            post_exec,
            observers,
        })
    }

    #[cfg(feature = "systemmode")]
    pub fn break_on_timeout(&mut self) {
        super::break_on_timeout();
    }

    /// Retrieve the emulator, consuming the executor.
    #[inline]
    #[must_use]
    pub fn into_emulator(self) -> EMU {
        self.emulator
    }
}

impl<EMU, OT, PRE, POST> DependencyResolver for StdQemuExecutor<EMU, OT, PRE, POST> {}

impl<EMU, I, OT, PRE, POST, S> Executor<I, S> for StdQemuExecutor<EMU, OT, PRE, POST>
where
    EMU: Emulator<Input = I, State = S>,
    OT: ObserversTuple<S>,
    PRE: FnMut(&mut S, &I, &mut EMU) -> Result<()>,
    POST: FnMut(&mut S, &I, &mut EMU, &mut ExitKind) -> Result<()>,
{
    type Observers = OT;

    fn init<W: Worker>(
        &mut self,
        state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        self.emulator.first_exec(state)
    }

    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind> {
        (self.pre_exec)(state, input, &mut self.emulator)?;

        self.emulator.pre_exec(state, input)?;

        let mut exit_kind = self.emulator.exec_input(input)?;

        (self.post_exec)(state, input, &mut self.emulator, &mut exit_kind)?;

        self.emulator
            .post_exec(state, input, &mut self.observers, &mut exit_kind)?;

        Ok(exit_kind)
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }

    #[cfg(feature = "systemmode")]
    unsafe fn handle_crash(
        &mut self,
        _state: &mut S,
        _input: Option<&I>,
        _params: &OsTerminationParams,
    ) -> Result<CrashStatus> {
        log::error!("Crash in QEMU systemmode: this is a fuzzer bug.");
        Ok(CrashStatus::FuzzerCrash)
    }

    #[cfg(feature = "usermode")]
    unsafe fn handle_crash(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        params: &OsTerminationParams,
    ) -> Result<CrashStatus> {
        unsafe {
            super::handle_crash(
                &mut self.emulator,
                &mut self.observers,
                state,
                input,
                params,
            )
        }
    }

    unsafe fn handle_timeout(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        _params: &libaflmm::runtimes::OsTerminationParams,
    ) -> Result<TimeoutStatus> {
        unsafe { super::handle_timeout(&mut self.emulator, &mut self.observers, state, input) }
    }
}
