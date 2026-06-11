use serde::Serialize;

use crate::{
    Result,
    common::{CompatibilityChecker, DependencyResolver, Registrator},
    controllers::{NopWorker, Worker},
    runtimes::{
        NopRuntime, OsTerminationParams, Runtime, TerminationHandlerData,
        inprocess::{CrashStatus, TimeoutStatus},
        restarting::LIBAFLMM_EXIT_RESTART,
        utils::{PinnedPtr, unix::OsShmSender},
    },
    states::State,
};
use core::{pin::Pin, ptr::NonNull, time::Duration};
use std::process::exit;

/// Object enabling interacting with a runtime's environment from the task.
/// It can be used to perform runtime-level operations generically.
///
/// It does not expose the runtime directly
#[derive(Debug)]
pub struct RuntimeHandle<S, W> {
    runtime: NonNull<dyn Runtime<S, W>>,
    worker: W,
    termination_data_ptr: Option<PinnedPtr<TerminationHandlerData>>,
    state_shm_sender: Option<OsShmSender<S>>,
}

impl<S> RuntimeHandle<S, NopWorker> {
    /// Create an empty runtime handle
    ///
    /// # Safety
    ///
    /// The inner runtime is a dangling pointer, it's unsafe to use it.
    #[must_use]
    pub unsafe fn empty(_state: &S) -> Self {
        let worker = NopWorker::default();

        Self {
            runtime: NonNull::<NopRuntime>::dangling(),
            worker,
            termination_data_ptr: None,
            state_shm_sender: None,
        }
    }
}

impl<S, W> RuntimeHandle<S, W> {
    pub(crate) unsafe fn new(runtime: *mut dyn Runtime<S, W>, worker: W) -> Self {
        Self {
            runtime: NonNull::new(runtime).expect("runtime ptr must be non-null"),
            worker,
            termination_data_ptr: None,
            state_shm_sender: None,
        }
    }

    unsafe fn runtime(&self) -> &dyn Runtime<S, W> {
        unsafe { self.runtime.as_ref() }
    }

    unsafe fn runtime_mut(&mut self) -> &mut dyn Runtime<S, W> {
        unsafe { self.runtime.as_mut() }
    }

    /// Set a timeout value for the runtime.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        unsafe { self.runtime_mut().set_timeout(timeout) }
    }

    /// Arm the [`Runtime`]'s timeout.
    pub fn arm_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().arm_timeout() }
    }

    /// Disarm the [`Runtime`]'s timeout.
    pub fn disarm_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().disarm_timeout() }
    }

    /// Unset a previously set timeout.
    ///
    /// If no timeout has been set before, it's a no-op.
    pub fn unset_timeout(&mut self) -> Result<()> {
        unsafe { self.runtime_mut().unset_timeout() }
    }

    /// Set the termination handler (used by the [`InProcessRuntime`]).
    ///
    /// # Safety
    ///
    /// `termination_data` must outlive this [`RuntimeHandle`].
    pub unsafe fn early_init_termination_handler(
        &mut self,
        mut termination_data: Pin<&mut TerminationHandlerData>,
        state: &mut S,
    ) where
        S: State,
        W: Worker,
    {
        assert!(
            self.termination_data_ptr.is_none(),
            "Termination data pointer has already been set. This is a fuzzer bug."
        );

        unsafe {
            TerminationHandlerData::commit_global(termination_data.as_mut());
        }

        let mut termination_data = PinnedPtr::from_pin(termination_data);
        let rt_handle_ptr = NonNull::from_mut(self);

        termination_data.early_init(state, rt_handle_ptr);

        self.termination_data_ptr = Some(termination_data);
    }

    /// Set the shared memory saver (used by the [`RestartingRuntime`]).
    pub fn set_saver(&mut self, state_shm_sender: OsShmSender<S>) {
        assert!(
            self.state_shm_sender.is_none(),
            "A state shm sender is already set in the runtime handle. This is a fuzzer bug."
        );

        self.state_shm_sender = Some(state_shm_sender);
    }

    /// Initialize the termination global data and handlers.
    pub fn init_termination_handlers<Z>(
        &mut self,
        fuzzer: &mut Z,
        on_crash: fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<CrashStatus>,
        on_timeout: fn(&mut TerminationHandlerData, &OsTerminationParams) -> Result<TimeoutStatus>,
    ) {
        if let Some(termination_data) = self.termination_data_ptr.as_mut() {
            termination_data.init(fuzzer, on_crash, on_timeout);

            if let Some(ref mut saver) = self.state_shm_sender {
                termination_data.set_saver_ptr(saver);
            }
        }
    }

    /// Set the input being run.
    pub fn set_input<I>(&mut self, input: &I) {
        if let Some(signal_data) = self.termination_data_ptr.as_mut() {
            signal_data.set_input(input);
        }
    }

    /// Clear the input being run.
    pub fn clear_input(&mut self) {
        if let Some(signal_data) = self.termination_data_ptr.as_mut() {
            signal_data.clear_input();
        }
    }

    /// Get a reference to the [`Worker`].
    pub fn worker(&self) -> &W {
        &self.worker
    }

    /// Get a mutable reference to the [`Worker`].
    pub fn worker_mut(&mut self) -> &mut W {
        &mut self.worker
    }

    /// Restart the current worker
    ///
    /// # Safety
    ///
    /// This will only work if the runtime is a restarting runtime.
    pub unsafe fn restart(&mut self, state: &mut S) -> !
    where
        S: Serialize,
    {
        if let Some(termination) = self.termination_data_ptr.as_mut()
            && let Some(saver) = unsafe { termination.saver() }
        {
            saver.send(state).expect("State save failed");
        }

        exit(LIBAFLMM_EXIT_RESTART)
    }
}

impl<S, W> DependencyResolver for RuntimeHandle<S, W> {
    fn check(&self, checker: &CompatibilityChecker) -> Result<()> {
        unsafe { self.runtime().check(checker) }
    }

    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_impl(registrator)?;

        unsafe { self.runtime_mut().register(registrator) }
    }
}
