use std::marker::PhantomData;

use crate::Result;
use crate::{emu::Emulator, qemu::Qemu};
use libaflmm::runtimes::{OsTerminationParams, inprocess::CrashStatus};
use libaflmm::{
    common::DependencyResolver,
    controllers::Worker,
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    runtimes::{RuntimeHandle, inprocess::TimeoutStatus},
};
use libaflmm_bolts::tuples::RefIndexable;

pub struct SimpleQemuExecutor<EMU, H, I, OT, S> {
    emulator: EMU,
    harness: H,
    observers: OT,
    phantom: PhantomData<(I, S)>,
}

impl<EMU, H, I, OT, S> SimpleQemuExecutor<EMU, H, I, OT, S> {
    pub fn new(_state: &mut S, emu: EMU, harness: H, observers: OT) -> Result<Self>
    where
        EMU: Emulator<I, S>,
        H: FnMut(&mut S, &I, Qemu) -> Result<ExitKind>,
    {
        Ok(Self {
            emulator: emu,
            harness,
            observers,
            phantom: PhantomData,
        })
    }

    #[cfg(feature = "systemmode")]
    pub fn break_on_timeout(&mut self) {
        super::break_on_timeout()
    }
}

impl<EMU, H, I, OT, S> DependencyResolver for SimpleQemuExecutor<EMU, H, I, OT, S> {}

impl<EMU, H, I, OT, S> Executor<I, S> for SimpleQemuExecutor<EMU, H, I, OT, S>
where
    EMU: Emulator<I, S>,
    OT: ObserversTuple<S>,
    H: FnMut(&mut S, &I, Qemu) -> Result<ExitKind>,
{
    type Observers = OT;

    fn init<W: Worker>(
        &mut self,
        state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> libaflmm::Result<()> {
        Ok(self.emulator.first_exec(state)?)
    }

    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> libaflmm::Result<ExitKind> {
        self.emulator.pre_exec(state, input)?;

        let mut exit_kind = (self.harness)(state, input, self.emulator.qemu())?;

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
    ) -> libaflmm::Result<CrashStatus> {
        log::error!("Crash in QEMU systemmode: this is a fuzzer bug.");
        Ok(CrashStatus::FuzzerCrash)
    }

    #[cfg(feature = "usermode")]
    unsafe fn handle_crash(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        params: &OsTerminationParams,
    ) -> libaflmm::Result<CrashStatus> {
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
    ) -> libaflmm::Result<TimeoutStatus> {
        unsafe { super::handle_timeout(&mut self.emulator, &mut self.observers, state, input) }
    }
}
