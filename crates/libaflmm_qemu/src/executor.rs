//! A `QEMU`-based executor for binary-only instrumentation in `LibAFL`

#[cfg(feature = "usermode")]
use crate::Qemu;
#[cfg(feature = "usermode")]
use crate::QemuSignalContext;

use crate::Emulator;
use libaflmm::{
    DependencyResolver, Result, Worker,
    executors::{Executor, ExitKind},
    observers::ObserversTuple,
    runtimes::{
        OsTerminationParams, RuntimeHandle,
        inprocess::{CrashStatus, TimeoutStatus},
    },
};
#[cfg(feature = "usermode")]
use libaflmm_bolts::minibsod;
use libaflmm_bolts::tuples::RefIndexable;
#[cfg(feature = "systemmode")]
use libaflmm_qemu_sys::libafl_exit_request_timeout;
#[cfg(feature = "usermode")]
use std::str;
#[cfg(feature = "systemmode")]
use std::sync::atomic::{AtomicBool, Ordering};

pub struct QemuExecutor<EMU, OT, PRE, POST> {
    emulator: EMU,
    pre_exec: PRE,
    post_exec: POST,
    observers: OT,
}

#[cfg(feature = "systemmode")]
pub(crate) static BREAK_ON_TMOUT: AtomicBool = AtomicBool::new(false);

impl<EMU, OT, PRE, POST> QemuExecutor<EMU, OT, PRE, POST> {
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
        BREAK_ON_TMOUT.store(true, Ordering::Release);
    }

    /// Retrieve the emulator, consuming the executor.
    #[inline]
    #[must_use]
    pub fn into_emulator(self) -> EMU {
        self.emulator
    }
}

impl<EMU, OT, PRE, POST> DependencyResolver for QemuExecutor<EMU, OT, PRE, POST> {}

impl<EMU, I, OT, PRE, POST, S> Executor<I, S> for QemuExecutor<EMU, OT, PRE, POST>
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
        let (signal, mut info, mut context) = match params {
            OsTerminationParams::Signal {
                signal,
                siginfo,
                context,
            } => (*signal, **siginfo, context.copied()),
            OsTerminationParams::Panic(panic) => panic!(
                "Panic termination ended up in QEMU crash handler, this is not expected: {panic:?}"
            ),
        };

        log::debug!("QEMU signal handler has been triggered (signal {signal})");

        if let Some(qemu) = Qemu::get() {
            // QEMU is already initialized, we have to route the signal to QEMU's handler or
            // consider it as a host (i.e. fuzzer) signal

            if qemu.is_running() {
                // QEMU is running, we must determine if we are coming from qemu's signal handler or not
                log::debug!("Signal has been triggered while QEMU was running");

                match qemu.signal_ctx() {
                    QemuSignalContext::OutOfQemuSignalHandler => {
                        // we did not run QEMU's signal handler, run it now
                        log::debug!("It's a simple signal, let QEMU handle it first");

                        unsafe {
                            qemu.run_signal_handler(signal.into(), &mut info, context.as_mut());
                        }

                        panic!("QEMU should have handled the signal handler by now");
                    }
                    QemuSignalContext::InQemuSignalHandlerHost => {
                        // we are running in a nested signal handling
                        // and the signal is a host QEMU signal

                        let si_addr = unsafe { info.si_addr() as usize };
                        log::error!(
                            "QEMU Host crash crashed at addr 0x{si_addr:x}... Bug in QEMU or Emulator modules? Exiting.\n"
                        );

                        if let Some(cpu) = qemu.current_cpu() {
                            eprint!("QEMU Context:\n{}", cpu.display_context());
                        }

                        Ok(CrashStatus::FuzzerCrash)
                    }
                    QemuSignalContext::InQemuSignalHandlerTarget => {
                        // we are running in a nested signal handler and the signal is a target signal.
                        // run qemu hooks then report the crash.

                        log::debug!(
                            "QEMU Target signal received that should be handled by host. It is a target crash."
                        );

                        self.emulator.post_exec(
                            state,
                            input.unwrap(),
                            &mut self.observers,
                            &mut ExitKind::Crash,
                        )?;

                        if let Some(cpu) = qemu.current_cpu() {
                            eprint!("QEMU Context:\n{}", cpu.display_context());
                        }

                        Ok(CrashStatus::TargetCrash)
                    }
                }
            } else {
                // qemu is not running, it is a bug in LibAFL
                let si_addr = unsafe { info.si_addr() as usize };
                log::error!(
                    "The fuzzer crashed at addr 0x{si_addr:x}... Bug in the fuzzer? Exiting."
                );

                let bsod = minibsod::generate_minibsod_to_vec(signal, &info, context.as_ref());

                if let Ok(bsod) = bsod {
                    if let Ok(bsod_str) = str::from_utf8(&bsod) {
                        log::error!("\n{bsod_str}");
                    } else {
                        log::error!("convert minibsod to string failed");
                    }
                } else {
                    log::error!("generate_minibsod failed");
                }

                Ok(CrashStatus::FuzzerCrash)
            }
        } else {
            // QEMU not initialized, this must be a fuzzer crash
            //
            // We should never end up there as the input should be set
            // before QEMU is initialized
            log::error!(
                "QEMU crash handler hit before being initialized. This should never happen."
            );
            Ok(CrashStatus::FuzzerCrash)
        }
    }

    unsafe fn handle_timeout(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        _params: &libaflmm::runtimes::OsTerminationParams,
    ) -> Result<TimeoutStatus> {
        self.emulator.post_exec(
            state,
            input.unwrap(),
            &mut self.observers,
            &mut ExitKind::Timeout,
        )?;

        #[cfg(feature = "systemmode")]
        unsafe {
            if BREAK_ON_TMOUT.load(Ordering::Acquire) {
                libafl_exit_request_timeout();
                return Ok(TimeoutStatus::Resume);
            }
        }

        Ok(TimeoutStatus::Exit)
    }
}
