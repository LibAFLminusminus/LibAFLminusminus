//! Termination is a generic term to talk about an abnormal program end, i.e. crash and timeout.

#[cfg(unix)]
use crate::runtimes::utils::unix::OsShmSender;
use crate::runtimes::{RuntimeHandle, utils::OsTerminationParams};
use core::{ffi::c_void, ptr::NonNull};
#[cfg(unix)]
use libafl_core::Error;

pub trait IntoTerminationHandlerData {
    fn as_termination_handler_data(&mut self) -> Option<NonNull<TerminationHandlerData>>;
}

#[derive(Debug, Clone)]
pub struct TerminationHandler<CH, D, TH> {
    termination_handler_depth: usize,
    termination_handler_max_depth: usize,
    pub(crate) crash_handler: CH,
    pub(crate) timeout_handler: TH,
    pub(crate) termination_data: D,
}

#[derive(Debug, Clone)]
pub struct TerminationHandlerData {
    state_ptr: Option<NonNull<c_void>>,
    input_ptr: Option<NonNull<c_void>>,
    observers_ptr: Option<NonNull<c_void>>,
    fuzzer_ptr: Option<NonNull<c_void>>,
    state_sender_ptr: Option<NonNull<c_void>>,
    crash_handler: Option<fn(&mut Self, &OsTerminationParams)>,
    timeout_handler: Option<fn(&mut Self, &OsTerminationParams)>,
}

unsafe impl Send for TerminationHandlerData {}
unsafe impl Sync for TerminationHandlerData {}

impl TerminationHandlerData {
    pub fn new() -> Self {
        Self {
            state_ptr: None,
            input_ptr: None,
            observers_ptr: None,
            fuzzer_ptr: None,
            state_sender_ptr: None,
            crash_handler: None,
            timeout_handler: None,
        }
    }

    pub fn init<O, S, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        observers: &mut O,
        on_crash: fn(&mut Self, &OsTerminationParams),
        on_timeout: fn(&mut Self, &OsTerminationParams),
    ) {
        if self.state_ptr.is_some() {
            panic!(
                "Trying to initialize termination information multiple times. This is a fuzzer bug."
            );
        }

        self.state_ptr = Some(NonNull::from(state).cast());
        self.fuzzer_ptr = Some(NonNull::from(fuzzer).cast());
        self.observers_ptr = Some(NonNull::from(observers).cast());
        self.crash_handler = Some(on_crash);
        self.timeout_handler = Some(on_timeout);
    }

    /// # Safety
    ///
    /// S must be the same as the one used during init
    pub unsafe fn state<S>(&self) -> &mut S {
        unsafe { self.state_ptr.unwrap().cast().as_mut() }
    }

    /// # Safety
    ///
    /// Z must be the same as the one used during init
    pub unsafe fn fuzzer<Z>(&self) -> &mut Z {
        unsafe { self.fuzzer_ptr.unwrap().cast().as_mut() }
    }

    /// # Safety
    ///
    /// O must be the same as the one used during init
    pub unsafe fn observers<O>(&self) -> &mut O {
        unsafe { self.observers_ptr.unwrap().cast().as_mut() }
    }

    /// # Safety
    ///
    /// I must be the same as the one used during set_input
    pub unsafe fn input<I>(&self) -> Option<&I> {
        unsafe { self.input_ptr.map(|input| input.cast().as_ref()) }
    }

    /// # Safety
    ///
    /// I must be the same as the one used during set_input
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
    #[cfg(unix)]
    pub unsafe fn saver<S>(&self) -> Option<&mut OsShmSender<S>> {
        unsafe { self.state_sender_ptr.map(|p| p.cast().as_mut()) }
    }

    #[cfg(unix)]
    pub(crate) fn set_saver_ptr<S>(&mut self, shm_sender: &mut OsShmSender<S>) {
        self.state_sender_ptr = Some(NonNull::from(shm_sender).cast());
    }

    pub fn set_input<I>(&mut self, input: &I) {
        self.input_ptr = Some(NonNull::from(input).cast());
    }

    pub fn clear_input(&mut self) {
        self.input_ptr = None;
    }

    pub fn in_fuzzing(&self) -> bool {
        self.input_ptr.is_some()
    }

    pub fn handle_crash(&mut self, termination_params: &OsTerminationParams) -> bool {
        if let Some(handler) = self.crash_handler {
            handler(self, termination_params);
            true
        } else {
            false
        }
    }

    pub fn handle_timeout(&mut self, termination_params: &OsTerminationParams) -> bool {
        if let Some(handler) = self.timeout_handler {
            handler(self, termination_params);
            true
        } else {
            false
        }
    }
}

impl IntoTerminationHandlerData for () {
    fn as_termination_handler_data(&mut self) -> Option<NonNull<TerminationHandlerData>> {
        None
    }
}

impl IntoTerminationHandlerData for TerminationHandlerData {
    fn as_termination_handler_data(&mut self) -> Option<NonNull<TerminationHandlerData>> {
        Some(NonNull::from(self))
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
    pub fn data(&self) -> &D {
        &self.termination_data
    }

    pub fn data_mut(&mut self) -> &mut D {
        &mut self.termination_data
    }
}

impl<CH, D, TH> TerminationHandler<CH, D, TH>
where
    CH: FnMut(&mut D, &OsTerminationParams) -> Result<(), Error>,
    TH: FnMut(&mut D, &OsTerminationParams) -> Result<(), Error>,
{
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

    pub fn termination_data_mut(&mut self) -> &mut D {
        &mut self.termination_data
    }
}
