use crate::{
    executors::common_signals,
    runtimes::{
        restarting::{
            LIBAFL_EXIT_CONTINUE, LIBAFL_EXIT_END, LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION,
        },
        utils::{IntoTerminationHandlerData, TerminationHandler},
    },
};
use alloc::{boxed::Box, vec::Vec};
use core::pin::Pin;
use libafl_bolts::os::{
    SIGNAL_RECURSION_EXIT, exit,
    unix_signals::{Signal, SignalHandler, setup_signal_handler, ucontext_t},
};
use libafl_core::Error;
use libc::{SIGABRT, siginfo_t};
use std::{
    backtrace::Backtrace,
    io::Write,
    panic::{self, PanicHookInfo},
};

pub type OsTerminationHandler<CH, D, TH> = UnixSignalHandler<CH, D, TH>;
pub type OsTerminationParams<'a> = SignalHandlerParams<'a>;

/// Wrapper to assert `Send + Sync` for a raw pointer.
///
/// # Safety
/// The caller must ensure the pointed-to data is only accessed
/// in a thread-safe manner.
struct SignalHandlerPtr<CH, D, TH> {
    signal_handler: *mut UnixSignalHandler<CH, D, TH>,
}

pub enum SignalHandlerParams<'a> {
    Signal {
        signal: &'a Signal,
        siginfo: &'a siginfo_t,
        context: Option<&'a ucontext_t>,
    },
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

pub struct UnixSignalHandler<CH, D, TH> {
    inner: TerminationHandler<CH, D, TH>,
}

pub(crate) type SignalHandlerFn<CH, D, TH> = unsafe fn(
    Signal,
    &mut siginfo_t,
    Option<&mut ucontext_t>,
    signal_handler: TerminationHandler<CH, D, TH>,
);

impl<CH, D, TH> UnixSignalHandler<CH, D, TH>
where
    for<'a> CH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<(), Error>
        + Send
        + Sync
        + Unpin
        + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    for<'a> TH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<(), Error>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    pub fn new(signal_handler: TerminationHandler<CH, D, TH>) -> Self {
        Self {
            inner: signal_handler,
        }
    }

    /// # Safety
    ///
    /// `signal_rt_handle` must contain `self`.
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
        signal: &Signal,
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
                    "The in process signal handler has been triggered {} times recursively, which is not expected. Exiting with error code {SIGNAL_RECURSION_EXIT}...",
                    self.inner().max_depth()
                );

                libc::exit(LIBAFL_EXIT_TERMINATION_INFINITE_RECURSION);
            }

            if self
                .inner
                .data_mut()
                .as_termination_handler_data()
                .map(|p| p.as_ref().in_fuzzing())
                .unwrap_or(false)
            {
                (self.inner.timeout_handler)(&mut self.inner.termination_data, &signal_params);

                exit(LIBAFL_EXIT_CONTINUE);
            } else {
                log::error!("Timeout out of fuzzing target. This is a fuzzer bug.");

                // offset by 128 to signal a fuzzer crash
                exit(128 + (*signal as i32));
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
        signal: &Signal,
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
                .map(|p| p.as_ref().in_fuzzing())
                .unwrap_or(false)
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

                (self.inner.crash_handler)(&mut self.inner.termination_data, &signal_params)
                    .expect("Error while handling crash handler");

                exit(LIBAFL_EXIT_CONTINUE);
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
            exit(128 + (*signal as i32));
        }
    }

    pub fn inner(&self) -> &TerminationHandler<CH, D, TH> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut TerminationHandler<CH, D, TH> {
        &mut self.inner
    }

    pub fn setup_panic_hook(self: &mut Pin<Box<Self>>) {
        let old_hook = panic::take_hook();

        let signal_handler_ptr: SignalHandlerPtr<CH, D, TH> = unsafe {
            SignalHandlerPtr::new(Pin::as_mut(self).get_mut() as *mut UnixSignalHandler<CH, D, TH>)
        };

        // # Safety
        // The panic handler should only run when all other execution stopped.
        // At this point, accessing the global state should be sound.
        panic::set_hook(Box::new(move |panic_info| unsafe {
            let signal_params = OsTerminationParams::Panic(panic_info);

            old_hook(panic_info);

            let signal_handler: &mut Self = &mut *signal_handler_ptr.as_mut_ptr();

            let max_depth_reached = signal_handler.enter();

            if max_depth_reached {
                log::error!(
                    "The in process signal handler has been triggered {} times recursively, which is not expected. Exiting with error code {SIGNAL_RECURSION_EXIT}...",
                    signal_handler.inner.max_depth()
                );

                libc::exit(SIGNAL_RECURSION_EXIT);
            }

            if !signal_handler
                .inner
                .termination_data
                .as_termination_handler_data()
                .map(|p| p.as_ref().in_fuzzing())
                .unwrap_or(false)
            {
                log::error!("Fuzzer panicked out of the fuzzing loop. This is a Fuzzer bug.");
                libafl_bolts::os::exit(128 + SIGABRT);
                return;
            }

            // fuzzing in progress, propagate crash
            log::error!("Target panicked");
            let backtrace = Backtrace::force_capture();
            eprintln!("stack backtrace:\n{backtrace}");

            (signal_handler.inner.crash_handler)(
                &mut signal_handler.inner.termination_data,
                &signal_params,
            );

            libafl_bolts::os::exit(LIBAFL_EXIT_CONTINUE);
        }));
    }
}

impl<CH, D, TH> SignalHandler for UnixSignalHandler<CH, D, TH>
where
    for<'a> CH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<(), Error>
        + Send
        + Sync
        + Unpin
        + 'static,
    D: IntoTerminationHandlerData + Send + Sync + Unpin + 'static,
    for<'a> TH: FnMut(&mut D, &OsTerminationParams<'a>) -> Result<(), Error>
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
                    self.inner.max_depth()
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
