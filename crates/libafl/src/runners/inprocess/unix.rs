use alloc::{boxed::Box, vec::Vec};
use core::pin::Pin;
use libafl_core::Error;
use std::{io::Write, panic};

use libafl_bolts::os::{
    SIGNAL_RECURSION_EXIT,
    unix_signals::{Signal, SignalHandler, setup_signal_handler, ucontext_t},
};
use libc::siginfo_t;

use crate::{executors::common_signals, runners::inprocess::InProcessSignalHandler};

pub type OsSignalHandler<CH, D, TH> = UnixSignalHandler<CH, D, TH>;

pub struct UnixSignalHandler<CH, D, TH> {
    inner: InProcessSignalHandler<CH, D, TH>,
}

pub(crate) type SignalHandlerFn<CH, D, TH> = unsafe fn(
    Signal,
    &mut siginfo_t,
    Option<&mut ucontext_t>,
    signal_handler: InProcessSignalHandler<CH, D, TH>,
);

impl<CH, D, TH> UnixSignalHandler<CH, D, TH> {
    pub fn new(signal_handler: InProcessSignalHandler<CH, D, TH>) -> Self {
        Self {
            inner: signal_handler,
        }
    }

    /// # Safety
    ///
    /// `signal_driver` must contain `self`.
    pub fn init(self: &mut Pin<Box<Self>>) -> Result<(), Error> {
        self.setup_panic_hook();

        unsafe { setup_signal_handler(self.as_mut().get_mut() as *mut Self) }
    }

    pub fn enter(&mut self) -> bool {
        self.inner.enter()
    }

    pub fn exit(&mut self) {
        self.inner.exit()
    }

    /// Timeout-Handler for in-process fuzzing.
    /// It will store the current State to shmem, then exit.
    ///
    /// # Safety
    /// Well, signal handling is not safe
    pub fn timeout_handler(
        &mut self,
        _signal: &Signal,
        _siginfo: &siginfo_t,
        _context: Option<&ucontext_t>,
    ) {
        unsafe {
            let max_depth_reached = self.enter();

            if max_depth_reached {
                log::error!(
                    "The in process signal handler has been triggered {} times recursively, which is not expected. Exiting with error code {SIGNAL_RECURSION_EXIT}...",
                    self.inner().max_depth()
                );

                libc::exit(SIGNAL_RECURSION_EXIT);
            }

            if !self.inner().is_in_target() {
                log::warn!("TIMEOUT or SIGUSR2 happened, but currently not fuzzing.");
                return;
            }

            log::error!("Timeout in fuzz run.");

            self.inner().timeout_handler();

            libafl_bolts::os::exit(55);
        }
    }

    /// Crash-Handler for in-process fuzzing.
    /// Will be used for signal handling.
    /// It will store the current State to shmem, then exit.
    ///
    /// # Safety
    /// Well, signal handling is not safe
    pub fn crash_handler(
        &mut self,
        signal: &Signal,
        siginfo: &siginfo_t,
        context: Option<&ucontext_t>,
    ) {
        unsafe {
            if self.inner.is_in_target() {
                log::error!("Target crashed with signal {signal}");

                {
                    let mut bsod = Vec::new();
                    {
                        let mut writer = std::io::BufWriter::new(&mut bsod);
                        let bsod = libafl_bolts::minibsod::generate_minibsod(
                            &mut writer,
                            signal,
                            siginfo,
                            context.as_deref(),
                        );

                        if bsod.is_err() {
                            log::error!("generate_minibsod failed");
                        }
                        let _ = writer.flush();
                    }
                    if let Ok(r) = core::str::from_utf8(&bsod) {
                        log::error!("{r}");
                    }
                }

                self.inner.crash_handler()
            } else {
                #[cfg(target_os = "android")]
                let si_addr = (siginfo._pad[0] as i64) | ((_info._pad[1] as i64) << 32);
                #[cfg(not(target_os = "android"))]
                let si_addr = {
                    siginfo.si_addr() as usize;
                };

                log::error!(
                    "Fuzzer crashed at addr 0x{si_addr:x}, but not in target. This is a fuzzer bug. Exiting."
                );

                {
                    let mut bsod = Vec::new();
                    {
                        let mut writer = std::io::BufWriter::new(&mut bsod);
                        let bsod = libafl_bolts::minibsod::generate_minibsod(
                            &mut writer,
                            signal,
                            siginfo,
                            context.as_deref(),
                        );
                        if bsod.is_err() {
                            log::error!("generate_minibsod failed");
                        }
                        let _ = writer.flush();
                    }
                    if let Ok(r) = core::str::from_utf8(&bsod) {
                        log::error!("{r}");
                    }
                }
            }

            // offset by 128 to signal a fuzzer crash
            libafl_bolts::os::exit(128 + (*signal as i32));
        }
    }

    pub fn inner(&self) -> &InProcessSignalHandler<CH, D, TH> {
        &self.inner
    }

    pub fn setup_panic_hook(self: &mut Pin<Box<Self>>) {
        let old_hook = panic::take_hook();

        let mut signal_handler: *mut Self = self.get_mut() as *mut UnixSignalHandler<CH, D, TH>;

        // # Safety
        // The panic handler should only run when all other execution stopped.
        // At this point, accessing the global state should be sound.
        panic::set_hook(Box::new(move |panic_info| unsafe {
            old_hook(panic_info);

            let signal_handler: &mut Self = &mut *signal_handler;

            let max_depth_reached = signal_handler.enter();

            if max_depth_reached {
                log::error!(
                    "The in process signal handler has been triggered {} times recursively, which is not expected. Exiting with error code {SIGNAL_RECURSION_EXIT}...",
                    signal_handler.inner().max_depth()
                );

                libc::exit(SIGNAL_RECURSION_EXIT);
            }

            if !signal_handler.inner().is_in_target() {
                log::warn!("panic hook called, but currently not fuzzing.");
                return;
            }

            self.inner().crash_handler();

            libafl_bolts::os::exit(128 + 6); // SIGABRT exit code
        }));
    }
}

impl<CH, D, TH> SignalHandler for UnixSignalHandler<CH, D, TH> {
    /// # Safety
    /// This will access global state.
    unsafe fn handle(
        &mut self,
        signal: Signal,
        info: &mut siginfo_t,
        context: Option<&mut ucontext_t>,
    ) {
        // # Safety
        // This runs in a signal handler, no other threads access these variables/borrows anymore.
        unsafe {
            let max_depth_reached = self.enter();

            if max_depth_reached {
                log::error!(
                    "The in process signal handler has been triggered {} times recursively, which is not expected. Exiting with error code {SIGNAL_RECURSION_EXIT}...",
                    self.inner().max_depth()
                );
                libc::exit(SIGNAL_RECURSION_EXIT);
            }

            match signal {
                Signal::SigUser2 | Signal::SigAlarm => {
                    self.timeout_handler(&signal, &*info, context.as_deref());
                }
                _ => {
                    self.crash_handler(&signal, &*info, context.as_deref());
                }
            }

            self.exit();
        }
    }

    fn signals(&self) -> Vec<Signal> {
        common_signals()
    }
}
