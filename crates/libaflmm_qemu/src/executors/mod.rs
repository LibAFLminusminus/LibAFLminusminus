use crate::emu::Emulator;
#[cfg(feature = "usermode")]
use crate::qemu::{Qemu, QemuSignalContext};
#[cfg(feature = "usermode")]
use libaflmm::runtimes::OsTerminationParams;
#[cfg(feature = "usermode")]
use libaflmm::runtimes::inprocess::CrashStatus;
use libaflmm::runtimes::inprocess::TimeoutStatus;
use libaflmm::{executors::ExitKind, observers::ObserversTuple};
#[cfg(feature = "usermode")]
use libaflmm_bolts::minibsod;
#[cfg(feature = "systemmode")]
use std::sync::atomic::{AtomicBool, Ordering};

pub mod simple;
pub use simple::SimpleQemuExecutor;

pub mod standard;
pub use standard::StdQemuExecutor;

#[cfg(feature = "systemmode")]
pub(crate) static BREAK_ON_TMOUT: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "systemmode")]
pub fn break_on_timeout() {
    BREAK_ON_TMOUT.store(true, Ordering::Release);
}

#[cfg(feature = "usermode")]
unsafe fn handle_crash<EMU, I, OT, S>(
    emulator: &mut EMU,
    observers: &mut OT,
    state: &mut S,
    input: Option<&I>,
    params: &OsTerminationParams,
) -> libaflmm::Result<CrashStatus>
where
    EMU: Emulator<I, S>,
    OT: ObserversTuple<S>,
{
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

                    emulator.post_exec(state, input.unwrap(), observers, &mut ExitKind::Crash)?;

                    if let Some(cpu) = qemu.current_cpu() {
                        eprint!("QEMU Context:\n{}", cpu.display_context());
                    }

                    Ok(CrashStatus::TargetCrash)
                }
            }
        } else {
            // qemu is not running, it is a bug in LibAFL
            let si_addr = unsafe { info.si_addr() as usize };
            log::error!("The fuzzer crashed at addr 0x{si_addr:x}... Bug in the fuzzer? Exiting.");

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
        log::error!("QEMU crash handler hit before being initialized. This should never happen.");
        Ok(CrashStatus::FuzzerCrash)
    }
}

unsafe fn handle_timeout<EMU, I, OT, S>(
    emulator: &mut EMU,
    observers: &mut OT,
    state: &mut S,
    input: Option<&I>,
) -> libaflmm::Result<TimeoutStatus>
where
    EMU: Emulator<I, S>,
    OT: ObserversTuple<S>,
{
    emulator.post_exec(state, input.unwrap(), observers, &mut ExitKind::Timeout)?;

    #[cfg(feature = "systemmode")]
    unsafe {
        if BREAK_ON_TMOUT.load(Ordering::Acquire) {
            libaflmm_qemu_sys::libafl_exit_request_timeout();
            return Ok(TimeoutStatus::Resume);
        }
    }

    Ok(TimeoutStatus::Exit)
}
