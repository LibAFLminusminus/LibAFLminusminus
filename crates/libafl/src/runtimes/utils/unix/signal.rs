//! Unix signal handling

use alloc::{boxed::Box, vec::Vec};
use core::pin::Pin;
use std::{
    backtrace::Backtrace,
    io::Write,
    panic::{self, PanicHookInfo},
};

use libafl_bolts::os::{
    exit,
    unix_signals::{Signal, SignalHandler, setup_signal_handler, ucontext_t},
};
use libafl_core::Result;
use libc::{SIGABRT, siginfo_t};

use crate::{
    executors::common_signals,
    runtimes::{
        inprocess::{CrashStatus, TimeoutStatus},
        restarting::{LIBAFL_EXIT_RESTART, LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION},
        utils::{IntoTerminationHandlerData, TerminationHandler},
    },
};

/// Unix termination (signal) handler.
pub type OsTerminationHandler<CH, D, TH> = UnixSignalHandler<CH, D, TH>;

/// Unix termination (signal) parameters.
pub type OsTerminationParams<'a> = SignalHandlerParams<'a>;

/// Wrapper to assert `Send + Sync` for a raw pointer.
///
/// # Safety
/// The caller must ensure the pointed-to data is only accessed
/// in a thread-safe manner.
struct SignalHandlerPtr<CH, D, TH> {
    signal_handler: *mut UnixSignalHandler<CH, D, TH>,
}

/// Signal handler parameters
#[derive(Debug)]
pub enum SignalHandlerParams<'a> {
    /// Signal handler parameters
    Signal {
        /// The signal
        signal: Signal,

        /// The signal infos
        siginfo: &'a siginfo_t,

        /// The signal context
        context: Option<&'a ucontext_t>,
    },

    /// Panic handler parameters
    Panic(&'a PanicHookInfo<'a>),
}

unsafe impl<CH, D, TH> Send for SignalHandlerPtr<CH, D, TH>
where
    CH: Send,
    D: Send,
    TH: Send,
{
}

unsafe impl<CH, D, TH> Sync for SignalHandlerPtr<CH, D, TH>
where
    CH: Send,
    D: Send,
    TH: Send,
{
}

impl<CH, D, TH> SignalHandlerPtr<CH, D, TH> {
    unsafe fn new(signal_handler: *mut UnixSignalHandler<CH, D, TH>) -> Self {
        Self { signal_handler }
    }

    fn as_mut_ptr(&self) -> *mut UnixSignalHandler<CH, D, TH> {
        self.signal_handler
    }
}

/// A Unix signal handler.
#[derive(Debug, Clone)]
pub struct UnixSignalHandler<CH, D, TH> {
    inner: TerminationHandler<CH, D, TH>,
}

impl<CH, D, TH> UnixSignalHandler<CH, D, TH>
where
    for<'a> CH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<CrashStatus>
        + Send
        + Sync
        + Unpin
        + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    for<'a> TH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<TimeoutStatus>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    /// Create a new [`UnixSignalHandler`].
    pub fn new(signal_handler: TerminationHandler<CH, D, TH>) -> Self {
        Self {
            inner: signal_handler,
        }
    }

    /// # Safety
    ///
    /// `signal_rt_handle` must contain `self`.
    pub fn init(self: &mut Pin<Box<Self>>) -> Result<()> {
        self.setup_panic_hook();

        unsafe { setup_signal_handler(core::ptr::from_mut::<Self>(self.as_mut().get_mut())) }
    }

    /// Called when entering a signal handler
    pub fn enter(&mut self) -> bool {
        self.inner.enter()
    }

    /// Called when exiting a signal handler
    pub fn exit(&mut self) {
        self.inner.exit();
    }

    /// Timeout-Handler for in-process fuzzing.
    /// It will store the current State to shmem, then exit.
    ///
    /// # Safety
    /// Well, signal handling is not safe
    pub fn timeout_handler(
        &mut self,
        signal: Signal,
        siginfo: &siginfo_t,
        context: Option<&ucontext_t>,
    ) {
        let signal_params = OsTerminationParams::Signal {
            signal,
            siginfo,
            context,
        };

        unsafe {
            let max_depth_reached = self.enter();

            if max_depth_reached {
                log::error!(
                    "The in process signal handler has been triggered {} times recursively (timeout handler), which is not expected. Exiting with error code {LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION}...",
                    self.inner().max_depth()
                );

                exit(LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION);
            }

            if self
                .inner
                .data_mut()
                .as_termination_handler_data()
                .is_some_and(|p| p.as_ref().in_fuzzing())
            {
                let status =
                    (self.inner.timeout_handler)(&mut self.inner.termination_data, &signal_params)
                        .expect("Error in timeout handler");

                match status {
                    TimeoutStatus::Exit => {
                        // timeout should exit
                        exit(LIBAFL_EXIT_RESTART);
                    }
                    TimeoutStatus::Resume => {
                        // resume the fuzzer on timeout
                        return;
                    }
                }
            } else {
                log::error!("Timeout out of fuzzing target. This is a fuzzer bug.");

                // offset by 128 to signal a fuzzer crash
                exit(128 + (signal as i32));
            }
        }
    }

    /// Crash-Handler for in-process fuzzing.
    /// Will be used for signal handling.
    ///
    /// # Safety
    ///
    /// Well, signal handling is not safe
    pub fn crash_handler(
        &mut self,
        signal: Signal,
        siginfo: &siginfo_t,
        context: Option<&ucontext_t>,
    ) {
        let signal_params = OsTerminationParams::Signal {
            signal,
            siginfo,
            context,
        };

        unsafe {
            if self
                .inner
                .data_mut()
                .as_termination_handler_data()
                .is_some_and(|p| p.as_ref().in_fuzzing())
            {
                // fuzzing in progress, propagate crash
                log::error!("Target crashed with signal {signal}");

                {
                    let mut bsod = Vec::new();
                    {
                        let mut writer = std::io::BufWriter::new(&mut bsod);
                        let bsod = libafl_bolts::minibsod::generate_minibsod(
                            &mut writer,
                            signal,
                            siginfo,
                            context,
                        );

                        if bsod.is_err() {
                            log::error!("generate_minibsod failed");
                        }
                        let _ = writer.flush();
                    }
                    if let Ok(r) = core::str::from_utf8(&bsod) {
                        log::error!("\n{r}");
                    }
                }

                (self.inner.crash_handler)(&mut self.inner.termination_data, &signal_params)
                    .expect("Error while handling crash handler");

                exit(LIBAFL_EXIT_RESTART);
            } else {
                // not in fuzzing loop, this is a fuzzer bug.
                let si_addr = { siginfo.si_addr() as usize };

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
                            context,
                        );
                        if bsod.is_err() {
                            log::error!("generate_minibsod failed");
                        }
                        let _ = writer.flush();
                    }
                    if let Ok(r) = core::str::from_utf8(&bsod) {
                        log::error!("\n{r}");
                    }
                }
            }

            // offset by 128 to signal a fuzzer crash
            exit(128 + (signal as i32));
        }
    }

    /// Reference to the inner [`TerminationHandler`].
    pub fn inner(&self) -> &TerminationHandler<CH, D, TH> {
        &self.inner
    }

    /// Mutable reference to the inner [`TerminationHandler`].
    pub fn inner_mut(&mut self) -> &mut TerminationHandler<CH, D, TH> {
        &mut self.inner
    }

    /// Setup the panic hook.
    pub fn setup_panic_hook(self: &mut Pin<Box<Self>>) {
        let old_hook = panic::take_hook();

        let signal_handler_ptr: SignalHandlerPtr<CH, D, TH> = unsafe {
            SignalHandlerPtr::new(core::ptr::from_mut::<UnixSignalHandler<CH, D, TH>>(Pin::as_mut(self).get_mut()))
        };

        // # Safety
        // The panic handler should only run when all other execution stopped.
        // At this point, accessing the global state should be sound.
        panic::set_hook(Box::new(move |panic_info| unsafe {
            let signal_params = OsTerminationParams::Panic(panic_info);

            let signal_handler: &mut Self = &mut *signal_handler_ptr.as_mut_ptr();

            let max_depth_reached = signal_handler.enter();

            if max_depth_reached {
                log::error!(
                    "The in process signal handler has been triggered {} times recursively (panic handler), which is not expected. Exiting with error code {LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION}...",
                    signal_handler.inner.max_depth()
                );

                exit(LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION);
            }

            if !signal_handler
                .inner
                .termination_data
                .as_termination_handler_data()
                .is_some_and(|p| p.as_ref().in_fuzzing())
            {
                // not in a fuzzing run: use the default hook (includes RUST_BACKTRACE output)
                old_hook(panic_info);
                log::error!("Fuzzer panicked out of the fuzzing loop. This is a Fuzzer bug.");
                exit(128 + SIGABRT);
            }

            // fuzzing in progress: print our own backtrace, skip the default hook
            log::error!("Target panicked: {panic_info}");
            let backtrace = Backtrace::force_capture();
            eprintln!("stack backtrace:\n{backtrace}");

            (signal_handler.inner.crash_handler)(
                &mut signal_handler.inner.termination_data,
                &signal_params,
            )
            .expect("Error in panic handler");

            exit(LIBAFL_EXIT_RESTART);
        }));
    }
}

impl<CH, D, TH> SignalHandler for UnixSignalHandler<CH, D, TH>
where
    for<'a> CH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<CrashStatus>
        + Send
        + Sync
        + Unpin
        + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    for<'a> TH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<TimeoutStatus>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    /// Signal handling entrypoint.
    ///
    /// # Safety
    ///
    /// This will access global state.
    /// No heap allocation should be performed there.
    unsafe fn handle(
        &mut self,
        signal: Signal,
        info: &mut siginfo_t,
        context: Option<&mut ucontext_t>,
    ) {
        let max_depth_reached = self.enter();

        if max_depth_reached {
            log::error!(
                "The in process signal handler has been triggered {} times recursively (crash handler), which is not expected. Exiting with error code {LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION}...",
                self.inner.max_depth()
            );
            exit(LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION);
        }

        match signal {
            Signal::SigUser2 | Signal::SigAlarm => {
                self.timeout_handler(signal, &*info, context.as_deref());
            }
            _ => {
                self.crash_handler(signal, &*info, context.as_deref());
            }
        }

        self.exit();
    }

    fn signals(&self) -> Vec<Signal> {
        common_signals()
    }
}
