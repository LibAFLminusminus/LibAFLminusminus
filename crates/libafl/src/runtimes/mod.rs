use core::{ffi::c_void, ptr::NonNull, time::Duration};

use libafl_bolts::Error;

use crate::DependencyResolver;

/// Type-erased data shared between the runtime and signal handlers.
///
/// Stable pointers (`state`, `fuzzer`) are set once via [`SignalHandlerData::init`].
/// Per-run pointers (`input`, `observers`) are set by the executor before each harness call
/// and cleared afterwards.
pub struct SignalHandlerData {
    state_ptr: Option<NonNull<c_void>>,
    input_ptr: Option<NonNull<c_void>>,
    observers_ptr: Option<NonNull<c_void>>,
    fuzzer_ptr: Option<NonNull<c_void>>,
    crash_handler: Option<fn(&mut Self)>,
    timeout_handler: Option<fn(&mut Self)>,
}

unsafe impl Send for SignalHandlerData {}
unsafe impl Sync for SignalHandlerData {}

impl SignalHandlerData {
    pub fn new() -> Self {
        Self {
            state_ptr: None,
            input_ptr: None,
            observers_ptr: None,
            fuzzer_ptr: None,
            crash_handler: None,
            timeout_handler: None,
        }
    }

    pub fn init<O, S, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        observers: &mut O,
        on_crash: fn(&mut Self),
        on_timeout: fn(&mut Self),
    ) {
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

    pub fn set_input<I>(&mut self, input: &I) {
        self.input_ptr = Some(NonNull::from(input).cast());
    }

    pub fn clear_input(&mut self) {
        self.input_ptr = None;
    }

    pub fn in_fuzzing(&self) -> bool {
        self.input_ptr.is_some()
    }

    pub fn handle_crash(&mut self) {
        if let Some(handler) = self.crash_handler {
            handler(self);
        }
    }

    pub fn handle_timeout(&mut self) {
        if let Some(handler) = self.timeout_handler {
            handler(self);
        }
    }
}

pub trait IntoSignalHandlerData {
    fn as_signal_handler_data(&mut self) -> Option<NonNull<SignalHandlerData>> {
        None
    }
}

impl IntoSignalHandlerData for () {}

impl IntoSignalHandlerData for SignalHandlerData {
    fn as_signal_handler_data(&mut self) -> Option<NonNull<SignalHandlerData>> {
        Some(NonNull::from(self))
    }
}

pub mod direct;
pub mod inprocess;
#[cfg(not(feature = "remove_me"))]
pub mod restarting;

/// Environment used to run a task
pub trait Runtime<CT, S>: DependencyResolver {
    /// Run the runtime.
    /// A runtime task is terminal: it is called only once and the runtime will immediately exit when the task returns.
    ///
    /// This trait function should NEVER be called by a user directly.
    /// The user is intended to use `run`, as it will always perform the right action.
    ///
    /// This function is only useful for trait writers to implement their custom [`runtime`].
    ///
    /// # Safety
    ///
    /// The rt_handle MUST be linked to the current runtime.
    /// Using a `rt_handle` that is not instanciated with self as the runtime will lead to Undefined Behaviour.
    /// Use [`Self::run`], this function should not need to be called directly.
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<CT, S>) -> Result<(), Error>;

    fn run(&mut self, controller: &mut CT) -> Result<(), Error>
    where
        Self: Sized + 'static,
    {
        let mut rt_handle =
            unsafe { RuntimeHandle::new(self as *mut Self as *mut dyn Runtime<CT, S>, controller) };

        unsafe { self.run_impl(&mut rt_handle) }
    }

    /// Set a timeout value for the runtime.
    ///
    /// Once set, [`on_timeout`] will be executed after the input duration.
    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        Ok(())
    }

    /// Arm the timer, with the value previously provided to `set_timeout`
    ///
    /// If no timeout has been set previously, it's a no-op.
    fn arm_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Disarm the timer if it has been previously armed with `arm_timeout`.
    ///
    /// If not timer has been armed previously, it's a no-op.
    fn disarm_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    fn unset_timeout(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Object enabling interacting with a runtime's environment from the task.
/// It can be used to perform runtime-level operations generically.
/// It does not expose the runtime directly
pub struct RuntimeHandle<'a, CT, S> {
    runtime: NonNull<dyn Runtime<CT, S>>,
    controller: &'a mut CT,
    signal_data: Option<NonNull<SignalHandlerData>>,
}

impl<'a, CT, S> RuntimeHandle<'a, CT, S> {
    unsafe fn new(runtime: *mut dyn Runtime<CT, S>, controller: &'a mut CT) -> Self {
        Self {
            runtime: NonNull::new(runtime).expect("runtime ptr must be non-null"),
            controller,
            signal_data: None,
        }
    }

    unsafe fn runtime(&self) -> &dyn Runtime<CT, S> {
        unsafe { self.runtime.as_ref() }
    }

    unsafe fn runtime_mut(&mut self) -> &mut dyn Runtime<CT, S> {
        unsafe { self.runtime.as_mut() }
    }

    /// Set a timeout value for the runtime.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        unsafe { self.runtime_mut().set_timeout(timeout.clone()) }
    }

    pub fn arm_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runtime_mut().arm_timeout() }
    }

    pub fn disarm_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runtime_mut().disarm_timeout() }
    }

    /// Unset a previously set timeout.
    /// If no timeout has been set before, it's a no-op.
    pub fn unset_timeout(&mut self) -> Result<(), Error> {
        unsafe { self.runtime_mut().unset_timeout() }
    }

    pub fn init_signal_handlers<O, Z>(
        &mut self,
        state: &mut S,
        fuzzer: &mut Z,
        observers: &mut O,
        on_crash: fn(&mut SignalHandlerData),
        on_timeout: fn(&mut SignalHandlerData),
    ) {
        if let Some(mut signal_data) = self.signal_data {
            unsafe {
                signal_data
                    .as_mut()
                    .init(state, fuzzer, observers, on_crash, on_timeout);
            }
        }
    }
}

impl<'a, CT, S> DependencyResolver for RuntimeHandle<'a, CT, S> {
    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        unsafe { self.runtime().check(checker) }
    }

    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        unsafe { self.runtime_mut().register(registrator) }
    }
}
