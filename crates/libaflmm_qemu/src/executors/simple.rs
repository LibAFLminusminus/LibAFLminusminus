use crate::{Emulator, Qemu};
use libaflmm::runtimes::{OsTerminationParams, inprocess::CrashStatus};
use libaflmm::{
    Result,
    common::DependencyResolver,
    controllers::Worker,
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    runtimes::{RuntimeHandle, inprocess::TimeoutStatus},
};
use libaflmm_bolts::tuples::RefIndexable;

pub struct SimpleQemuExecutor<EMU, H, OT> {
    emulator: EMU,
    harness: H,
    observers: OT,
}

impl<EMU, H, OT> SimpleQemuExecutor<EMU, H, OT> {
    pub fn new(_state: &mut EMU::State, emu: EMU, harness: H, observers: OT) -> Result<Self>
    where
        EMU: Emulator,
        H: FnMut(&mut EMU::State, &EMU::Input, Qemu) -> Result<ExitKind>,
    {
        Ok(Self {
            emulator: emu,
            harness,
            observers,
        })
    }

    #[cfg(feature = "systemmode")]
    pub fn break_on_timeout(&mut self) {
        super::break_on_timeout()
    }
}

impl<EMU, H, OT> DependencyResolver for SimpleQemuExecutor<EMU, H, OT> {}

impl<EMU, H, I, OT, S> Executor<I, S> for SimpleQemuExecutor<EMU, H, OT>
where
    EMU: Emulator<Input = I, State = S>,
    OT: ObserversTuple<S>,
    H: FnMut(&mut S, &I, Qemu) -> Result<ExitKind>,
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
