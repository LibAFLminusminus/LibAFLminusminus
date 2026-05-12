//! Termination is a generic term to talk about an abnormal program end, i.e. crash and timeout.

use crate::runtimes::inprocess::{CrashStatus, TimeoutStatus};
use crate::runtimes::utils::unix::OsShmSender;
use crate::{
    Fuzzer,
    executors::Executor,
    runtimes::{RuntimeHandle, utils::OsTerminationParams},
};
use core::pin::Pin;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};
use core::{ffi::c_void, ptr::NonNull};
use libafl_bolts::DebugUnwrap;
use libafl_core::Result;

/// Convertible into [`TerminationHandlerData`].
pub trait IntoTerminationHandlerData {
    /// Get the pinned [`TerminationHandlerData`] from the pinned wrapper
    fn termination_handler_data(self: Pin<&mut Self>) -> Option<Pin<&mut TerminationHandlerData>>;
}

static GLOBAL_TERMINATION_DATA: AtomicPtr<TerminationHandlerData> = AtomicPtr::new(ptr::null_mut());

/// Termination handlers
#[derive(Debug, Clone)]
pub struct TerminationHandler<CH, D, TH> {
    termination_handler_depth: usize,
    termination_handler_max_depth: usize,
    pub(crate) crash_handler: CH,
    pub(crate) timeout_handler: TH,
    pub(crate) termination_data: D,
}

/// Termination data for handlers in [`TerminationHandler`].
#[derive(Debug, Clone)]
pub struct TerminationHandlerData {
    // Data
    state_ptr: Option<NonNull<c_void>>,
    input_ptr: Option<NonNull<c_void>>,
    executor_ptr: Option<NonNull<c_void>>,
    fuzzer_ptr: Option<NonNull<c_void>>,
    rt_handle_ptr: Option<NonNull<c_void>>,
    state_sender_ptr: Option<NonNull<c_void>>,

    // Handlers
    crash_handler: Option<fn(&mut Self, &OsTerminationParams) -> Result<CrashStatus>>,
    timeout_handler: Option<fn(&mut Self, &OsTerminationParams) -> Result<TimeoutStatus>>,
}

unsafe impl Send for TerminationHandlerData {}
unsafe impl Sync for TerminationHandlerData {}

impl Default for TerminationHandlerData {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminationHandlerData {
    /// Get a new [`TerminationHandlerData`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            state_ptr: None,
            input_ptr: None,
            executor_ptr: None,
            fuzzer_ptr: None,
            rt_handle_ptr: None,
            state_sender_ptr: None,
            crash_handler: None,
            timeout_handler: None,
        }
    }

    /// Initialize the handler data.
    pub fn init<E, I, R, S, ST, W, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle_ptr: NonNull<RuntimeHandle<S, W>>,
        on_crash: fn(&mut Self, &OsTerminationParams) -> Result<CrashStatus>,
        on_timeout: fn(&mut Self, &OsTerminationParams) -> Result<TimeoutStatus>,
    ) where
        E: Executor<I, S>,
        Z: Fuzzer<E, I, R, S, ST, W>,
    {
        assert!(
            self.state_ptr.is_none(),
            "Trying to initialize termination information multiple times. This is a fuzzer bug."
        );

        self.state_ptr = Some(NonNull::from(state).cast());
        self.fuzzer_ptr = Some(NonNull::from(fuzzer).cast());
        self.executor_ptr = Some(NonNull::from(executor).cast());
        self.rt_handle_ptr = Some(rt_handle_ptr.cast());
        self.crash_handler = Some(on_crash);
        self.timeout_handler = Some(on_timeout);
    }

    /// # Safety
    ///
    /// S must be the same as the one used during init
    /// In release mode, initialization is not checked.
    #[must_use]
    #[expect(clippy::mut_from_ref)]
    pub unsafe fn state<S>(&self) -> &mut S {
        debug_assert!(self.state_ptr.is_some(), "state_ptr is not initialized");
        unsafe { self.state_ptr.unwrap_debug().cast().as_mut() }
    }

    /// # Safety
    ///
    /// Z must be the same as the one used during init
    /// In release mode, initialization is not checked.
    #[must_use]
    #[expect(clippy::mut_from_ref)]
    pub unsafe fn fuzzer<Z>(&self) -> &mut Z {
        debug_assert!(self.fuzzer_ptr.is_some(), "fuzzer_ptr is not initialized");
        unsafe { self.fuzzer_ptr.unwrap_debug().cast().as_mut() }
    }

    /// # Safety
    ///
    /// O must be the same as the one used during init
    /// In release mode, initialization is not checked.
    #[must_use]
    #[expect(clippy::mut_from_ref)]
    pub unsafe fn executor<E, I, S>(&self) -> &mut E
    where
        E: Executor<I, S>,
    {
        unsafe { self.executor_ptr.unwrap_debug().cast().as_mut() }
    }

    /// # Safety
    ///
    /// S and W must be the same as the one used during init
    /// In release mode, initialization is not checked.
    #[must_use]
    #[expect(clippy::mut_from_ref)]
    pub unsafe fn rt_handle<S, W>(&self) -> &mut RuntimeHandle<S, W> {
        unsafe { self.rt_handle_ptr.unwrap_debug().cast().as_mut() }
    }

    /// # Safety
    ///
    /// I must be the same as the one used during `set_input`
    #[must_use]
    pub unsafe fn input<I>(&self) -> Option<&I> {
        unsafe { self.input_ptr.map(|input| input.cast().as_ref()) }
    }

    /// # Safety
    ///
    /// I must be the same as the one used during `set_input`
    pub unsafe fn take_input<I: Clone>(&mut self) -> Option<I> {
        unsafe {
            self.input_ptr.take().map(|input| {
                let input: &I = input.cast().as_ref();
                input.clone()
            })
        }
    }

    /// # Safety
    ///
    /// S must be the same type used when the saver was registered via `RuntimeHandle`.
    #[must_use]
    #[expect(clippy::mut_from_ref)]
    #[cfg(unix)]
    pub unsafe fn saver<S>(&self) -> Option<&mut OsShmSender<S>> {
        unsafe { self.state_sender_ptr.map(|p| p.cast().as_mut()) }
    }

    #[cfg(unix)]
    pub(crate) fn set_saver_ptr<S>(&mut self, shm_sender: &mut OsShmSender<S>) {
        self.state_sender_ptr = Some(NonNull::from(shm_sender).cast());
    }

    /// Set the data input.
    pub fn set_input<I>(&mut self, input: &I) {
        self.input_ptr = Some(NonNull::from(input).cast());
    }

    /// Clear the data input.
    pub fn clear_input(&mut self) {
        self.input_ptr = None;
    }

    /// Are we in target code?
    #[must_use]
    pub fn in_fuzzing(&self) -> bool {
        self.input_ptr.is_some()
    }

    /// Handle a crash.
    pub fn handle_crash(
        &mut self,
        termination_params: &OsTerminationParams,
    ) -> Result<CrashStatus> {
        (self.crash_handler.expect("No crash handler found"))(self, termination_params)
    }

    /// Handle a timeout.
    pub fn handle_timeout(
        &mut self,
        termination_params: &OsTerminationParams,
    ) -> Result<TimeoutStatus> {
        (self.timeout_handler.expect("No timeout handler found"))(self, termination_params)
    }

    /// Commit the global process state
    ///
    /// # Safety
    ///
    /// `Self` must outlive any call to `commit_global`.
    pub unsafe fn commit_global(self: Pin<&mut Self>) {
        let ptr = unsafe { Pin::into_inner_unchecked(self) } as *mut Self;
        GLOBAL_TERMINATION_DATA.store(ptr, Ordering::Release);
    }

    /// Get a reference to the process global state
    ///
    /// # Safety
    ///
    /// Committed data must still be alive.
    /// In release mode, previous commitment is not checked.
    pub unsafe fn global() -> &'static Self {
        let ptr = GLOBAL_TERMINATION_DATA.load(Ordering::Acquire);
        unsafe { ptr.as_ref().unwrap_debug() }
    }

    /// Get a mutable reference to the process global state
    ///
    /// # Safety
    ///
    /// Committed data must still be alive.
    /// In release mode, previous commitment is not checked.
    pub unsafe fn global_mut() -> &'static mut Self {
        let ptr = GLOBAL_TERMINATION_DATA.load(Ordering::Acquire);
        unsafe { ptr.as_mut().unwrap_debug() }
    }
}

impl IntoTerminationHandlerData for () {
    fn termination_handler_data(self: Pin<&mut Self>) -> Option<Pin<&mut TerminationHandlerData>> {
        None
    }
}

impl IntoTerminationHandlerData for TerminationHandlerData {
    fn termination_handler_data(self: Pin<&mut Self>) -> Option<Pin<&mut TerminationHandlerData>> {
        Some(self)
    }
}

unsafe impl<CH, D, TH> Send for TerminationHandler<CH, D, TH>
where
    CH: Send,
    D: Send,
    TH: Send,
{
}

unsafe impl<CH, D, TH> Sync for TerminationHandler<CH, D, TH>
where
    CH: Sync,
    D: Sync,
    TH: Sync,
{
}

impl<CH, D, TH> TerminationHandler<CH, D, TH> {
    /// Get the reference to the data of the termination handler.
    pub fn data(&self) -> &D {
        &self.termination_data
    }

    /// Get the mutable reference to the data of the termination handler.
    pub fn data_mut(&mut self) -> &mut D {
        &mut self.termination_data
    }
}

impl<CH, D, TH> TerminationHandler<CH, D, TH>
where
    CH: FnMut(&mut D, &OsTerminationParams) -> Result<CrashStatus>,
    TH: FnMut(&mut D, &OsTerminationParams) -> Result<TimeoutStatus>,
{
    /// Create a new [`TerminationHandler`].
    pub fn new(crash_handler: CH, termination_data: D, timeout_handler: TH) -> Self {
        Self {
            crash_handler,
            timeout_handler,
            termination_handler_depth: 0,
            termination_handler_max_depth: 3,
            termination_data,
        }
    }

    pub(crate) fn enter(&mut self) -> bool {
        self.termination_handler_depth += 1;

        self.termination_handler_depth >= self.termination_handler_max_depth
    }

    pub(crate) fn exit(&mut self) {
        self.termination_handler_depth -= 1;
    }

    pub(crate) fn max_depth(&self) -> usize {
        self.termination_handler_max_depth
    }

    /// Get a mutable reference to the termination data.
    pub fn termination_data_mut(&mut self) -> &mut D {
        &mut self.termination_data
    }
}
