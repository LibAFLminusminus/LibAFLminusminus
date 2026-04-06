//! The hook for `InProcessExecutor`
#[cfg(all(target_os = "linux", feature = "std"))]
use core::mem::zeroed;
#[cfg(any(unix, all(windows, feature = "std")))]
use core::sync::atomic::{Ordering, compiler_fence};
use core::{
    ffi::c_void,
    marker::PhantomData,
    ptr::{null, null_mut},
    time::Duration,
};

#[cfg(all(target_os = "linux", feature = "std"))]
use libafl_bolts::current_time;
#[cfg(all(unix, feature = "std"))]
use libafl_bolts::minibsod::{BsodInfo, generate_minibsod_to_vec};
#[cfg(all(unix, feature = "std", not(miri)))]
use libafl_bolts::os::unix_signals::setup_signal_handler;
#[cfg(all(windows, feature = "std"))]
use libafl_bolts::os::windows_exceptions::setup_exception_handler;
#[cfg(all(windows, feature = "std"))]
use windows::Win32::System::Threading::{CRITICAL_SECTION, PTP_TIMER};

#[cfg(feature = "std")]
use crate::executors::hooks::timer::TimerStruct;
use crate::{
    Error, HasObjective,
    executors::{Executor, hooks::ExecutorHook, inprocess::HasInProcessHooks},
    feedbacks::Feedback,
    state::{HasExecutions, HasSolutions},
};
#[cfg(all(unix, feature = "std"))]
use crate::{
    executors::{
        ExitKind, hooks::unix::unix_signal_handler, inprocess::run_observers_and_save_state,
    },
    state::HasCorpus,
};
#[cfg(any(unix, windows))]
use crate::{inputs::Input, observers::ObserversTuple, state::HasCurrentTestcase};

/// The inmem executor's handlers.
#[expect(missing_debug_implementations)]
pub struct InProcessHooks<I, S> {
    /// `Timer` struct
    #[cfg(feature = "std")]
    pub timer: TimerStruct,
    phantom: PhantomData<(I, S)>,
}

impl<I, S> ExecutorHook<I, S> for InProcessHooks<I, S> {
    fn init(&mut self, _state: &mut S) {}
    /// Call before running a target.
    fn pre_exec(&mut self, _state: &mut S, _input: &I) {
        #[cfg(feature = "std")]
        self.timer.set_timer();
    }

    /// Call after running a target.
    fn post_exec(&mut self, _state: &mut S, _input: &I) {
        #[cfg(feature = "std")]
        self.timer.unset_timer();
    }
}

impl<I, S> InProcessHooks<I, S> {
    /// Create new [`InProcessHooks`].
    #[cfg(unix)]
    #[allow(unused_variables)] // for `exec_tmout` without `std`
    pub fn new<E, EM, OF, Z>(exec_tmout: Duration) -> Result<Self, Error>
    where
        E: Executor<EM, I, S, Z> + HasObservers + HasInProcessHooks<I, S>,
        E::Observers: ObserversTuple<I, S>,
        EM: EventFirer<I, S> + EventRestarter<S>,
        OF: Feedback<EM, I, E::Observers, S>,
        S: HasExecutions + HasSolutions<I> + HasCurrentTestcase<I>,
        Z: HasObjective<Objective = OF>,
        I: Input + Clone,
    {
        // # Safety
        // We get a pointer to `GLOBAL_STATE` that will be initialized at this point in time.
        // This unsafe is needed in stable but not in nightly. Remove in the future(?)
        #[expect(unused_unsafe)]
        #[cfg(all(not(miri), unix, feature = "std"))]
        let data = unsafe { &raw mut GLOBAL_STATE };
        #[cfg(feature = "std")]
        unix_signal_handler::setup_panic_hook::<E, EM, I, OF, S, Z>();
        // # Safety
        // Setting up the signal handlers with a pointer to the `GLOBAL_STATE` which should not be NULL at this point.
        // We are the sole users of `GLOBAL_STATE` right now, and only dereference it in case of Segfault/Panic.
        // In that case we get the mutable borrow. Otherwise we don't use it.
        #[cfg(all(not(miri), unix, feature = "std"))]
        unsafe {
            setup_signal_handler(data)?;
        }

        // setup the pointer for the signal handlers.
        unsafe {
            let data = &raw mut GLOBAL_STATE;
            assert!((*data).crash_handler.is_null());
            // usually timeout handler and crash handler is set together
            // so no check for timeout handler is null or not
            (*data).crash_handler =
                unix_signal_handler::inproc_crash_handler::<E, EM, I, OF, S, Z> as *const c_void;
            (*data).timeout_handler =
                unix_signal_handler::inproc_timeout_handler::<E, EM, I, OF, S, Z> as *const _;
        }

        Ok(Self {
            #[cfg(feature = "std")]
            timer: TimerStruct::new(exec_tmout),
            phantom: PhantomData,
        })
    }

    /// Create new [`InProcessHooks`].
    #[cfg(windows)]
    #[allow(unused_variables)] // for `exec_tmout` without `std`
    pub fn new<E, EM, OF, Z>(exec_tmout: Duration) -> Result<Self, Error>
    where
        E: Executor<EM, I, S, Z> + HasObservers + HasInProcessHooks<I, S>,
        E::Observers: ObserversTuple<I, S>,
        EM: EventFirer<I, S> + EventRestarter<S>,
        I: Input + Clone,
        OF: Feedback<EM, I, E::Observers, S>,
        S: HasExecutions + HasSolutions<I> + HasCurrentTestcase<I>,
        Z: HasObjective<Objective = OF>,
    {
        let ret;
        #[cfg(feature = "std")]
        unsafe {
            let data = &raw mut GLOBAL_STATE;
            crate::executors::hooks::windows::windows_exception_handler::setup_panic_hook::<
                E,
                EM,
                I,
                OF,
                S,
                Z,
            >();
            setup_exception_handler(data)?;

            // setup the pointer for the signal handlers.
            unsafe {
                let data = &raw mut GLOBAL_STATE;
                assert!((*data).crash_handler.is_null());
                // usually timeout handler and crash handler is set together
                // so no check for timeout handler is null or not
                (*data).crash_handler =
                    unix_signal_handler::inproc_crash_handler::<E, EM, I, OF, S, Z>
                        as *const c_void;
                (*data).timeout_handler =
                    unix_signal_handler::inproc_timeout_handler::<E, EM, I, OF, S, Z> as *const _;
            }
            let timer = TimerStruct::new(exec_tmout, timeout_handler);
            ret = Ok(Self {
                timer,
                phantom: PhantomData,
            });
        }
        #[cfg(not(feature = "std"))]
        {
            ret = Ok(Self {
                phantom: PhantomData,
            });
        }

        ret
    }

    /// Replace the handlers with `nop` handlers, deactivating the handlers
    #[must_use]
    #[cfg(not(windows))]
    pub fn nop() -> Self {
        Self {
            timer: TimerStruct::new(Duration::from_secs(60)),
            phantom: PhantomData,
        }
    }
}

/// The global state of the in-process harness.
#[derive(Debug)]
pub struct SignalHandlerData {
    /// the pointer to the state
    pub state_ptr: *mut c_void,
    /// the pointer to the fuzzer
    pub fuzzer_ptr: *mut c_void,
    /// the pointer to the executor
    pub executor_ptr: *const c_void,
    pub(crate) current_input_ptr: *const c_void,

    #[cfg(feature = "std")]
    pub(crate) signal_handler_depth: usize,

    #[cfg(all(windows, feature = "std"))]
    pub(crate) ptp_timer: Option<PTP_TIMER>,
    #[cfg(all(windows, feature = "std"))]
    pub(crate) in_target: u64,
    #[cfg(all(windows, feature = "std"))]
    pub(crate) critical: *mut c_void,
}

unsafe impl Send for SignalHandlerData {}
unsafe impl Sync for SignalHandlerData {}

impl SignalHandlerData {
    #[cfg(feature = "std")]
    const SIGNAL_HANDLER_MAX_DEPTH: usize = 3;

    /// # Safety
    /// Only safe if not called twice and if the executor is not used from another borrow after this.
    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) unsafe fn executor_mut<'a, E>(&self) -> &'a mut E {
        unsafe { (self.executor_ptr as *mut E).as_mut().unwrap() }
    }

    /// # Safety
    /// Only safe if not called twice and if the state is not used from another borrow after this.
    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) unsafe fn state_mut<'a, S>(&self) -> &'a mut S {
        unsafe { (self.state_ptr as *mut S).as_mut().unwrap() }
    }

    /// # Safety
    /// Only safe if not called twice and if the fuzzer is not used from another borrow after this.
    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) unsafe fn fuzzer_mut<'a, Z>(&self) -> &'a mut Z {
        unsafe { (self.fuzzer_ptr as *mut Z).as_mut().unwrap() }
    }

    /// # Safety
    /// Only safe if not called concurrently.
    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) unsafe fn take_current_input<'a, I>(&mut self) -> &'a I {
        let r = unsafe { (self.current_input_ptr as *const I).as_ref().unwrap() };
        self.current_input_ptr = null();
        r
    }

    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) fn is_valid(&self) -> bool {
        !self.current_input_ptr.is_null()
    }

    /// Returns true if signal handling max depth has been reached, false otherwise
    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) fn signal_handler_enter(&mut self) -> (bool, usize) {
        self.signal_handler_depth += 1;
        (
            self.signal_handler_depth >= Self::SIGNAL_HANDLER_MAX_DEPTH,
            self.signal_handler_depth,
        )
    }

    #[cfg(all(feature = "std", any(unix, windows)))]
    pub(crate) fn signal_handler_exit(&mut self) {
        self.signal_handler_depth -= 1;
    }

    /// if data is valid, safely report a crash and return true.
    /// return false otherwise.
    ///
    /// # Safety
    ///
    /// Should only be called to signal a crash in the target
    #[cfg(all(unix, feature = "std"))]
    pub unsafe fn maybe_report_crash<E, EM, I, OF, S, Z>(
        &mut self,
        bsod_info: Option<BsodInfo>,
    ) -> bool
    where
        E: Executor<EM, I, S, Z> + HasObservers,
        E::Observers: ObserversTuple<I, S>,
        EM: EventFirer<I, S> + EventRestarter<S>,
        OF: Feedback<EM, I, E::Observers, S>,
        S: HasExecutions + HasSolutions<I> + HasCorpus<I> + HasCurrentTestcase<I>,
        Z: HasObjective<Objective = OF>,
        I: Input + Clone,
    {
        unsafe {
            if self.is_valid() {
                let executor = self.executor_mut::<E>();
                // disarms timeout in case of timeout
                let state = self.state_mut::<S>();
                let fuzzer = self.fuzzer_mut::<Z>();
                let input = self.take_current_input::<I>();

                log::error!("Target crashed!");

                if let Some(bsod_info) = bsod_info {
                    let bsod = generate_minibsod_to_vec(
                        bsod_info.signal,
                        &bsod_info.siginfo,
                        bsod_info.ucontext.as_ref(),
                    );

                    if let Ok(bsod) = bsod
                        && let Ok(r) = core::str::from_utf8(&bsod)
                    {
                        log::error!("{r}");
                    }
                }

                run_observers_and_save_state::<E, EM, I, OF, S, Z>(
                    executor,
                    state,
                    input,
                    fuzzer,
                    ExitKind::Crash,
                );

                return true;
            }

            false
        }
    }
}

unsafe fn prepare_exit<E, EM, I, OF, S, Z>(
    data: *mut InProcessExecutorHandlerData,
    exit_kind: ExitKind,
) where
    E: Executor<EM, I, S, Z> + HasObservers,
    E::Observers: ObserversTuple<I, S>,
    OF: Feedback<EM, I, E::Observers, S>,
    S: HasExecutions + HasSolutions<I> + HasCurrentTestcase<I>,
    Z: HasObjective<Objective = OF>,
    I: Input + Clone,
{
    unsafe {
        if (*data).is_valid() {
            let executor = (*data).executor_mut::<E>();
            let state = (*data).state_mut::<S>();
            let input = (*data).take_current_input::<I>();
            let fuzzer = (*data).fuzzer_mut::<Z>();

            run_observers_and_save_state::<E, EM, I, OF, S, Z>(
                executor, state, input, fuzzer, exit_kind,
            );
        }
    }
}

/// Exception handling needs some nasty globals.
pub(crate) static mut GLOBAL_STATE: SignalHandlerData = SignalHandlerData {
    // The state ptr for signal handling
    state_ptr: null_mut(),
    // The fuzzer ptr for signal handling
    fuzzer_ptr: null_mut(),
    // The executor ptr for signal handling
    executor_ptr: null(),
    // The current input for signal handling
    current_input_ptr: null(),

    #[cfg(feature = "std")]
    signal_handler_depth: 0,

    #[cfg(all(windows, feature = "std"))]
    ptp_timer: None,
    #[cfg(all(windows, feature = "std"))]
    in_target: 0,
    #[cfg(all(windows, feature = "std"))]
    critical: null_mut(),
};

/// Get the inprocess State
///
/// # Safety
/// This is a *very* dangerous and mainly for internal use (unless you _really_ know what you're doing.
/// The type is not checked and needs to be specified correctly.
/// Only safe if not called twice and if the state is not accessed from another borrow while this one is alive.
#[must_use]
pub unsafe fn inprocess_get_state<'a, S>() -> Option<&'a mut S> {
    // # Safety
    // As unsafe as it gets, but the function is documented accordingly.
    unsafe { (GLOBAL_STATE.state_ptr as *mut S).as_mut() }
}
