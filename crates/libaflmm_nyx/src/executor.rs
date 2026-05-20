use crate::{cmplog::CMPLOG_ENABLED, helper::NyxHelper};
use core::{ops::IndexMut, time::Duration};
use libaflmm::{
    Error,
    common::DependencyResolver,
    controllers::Worker,
    executors::{Executor, ExitKind},
    inputs::InputContext,
    observers::{ObserversTuple, StdOutObserver},
    states::State,
};
use libaflmm_bolts::{
    AsSlice,
    tuples::{Handle, RefIndexable},
};
use libnyx::NyxReturnValue;
use std::{
    io::{Read, Seek},
    os::fd::AsRawFd,
};

/// executor for nyx standalone mode
pub struct NyxExecutor<OT> {
    /// implement nyx function
    pub helper: NyxHelper,
    /// stdout
    stdout: Option<Handle<StdOutObserver>>,
    /// stderr
    // stderr: Option<StdErrObserver>,
    /// observers
    observers: OT,
    timeout: Option<Duration>,
}

impl NyxExecutor<()> {
    /// Create a builder for [`NyxExecutor`]
    #[must_use]
    pub fn builder() -> NyxExecutorBuilder {
        NyxExecutorBuilder::new()
    }
}

impl<OT> DependencyResolver for NyxExecutor<OT> {}

impl<I, OT, S> Executor<I, S> for NyxExecutor<OT>
where
    S: State<Input = I>,
    OT: ObserversTuple<S>,
{
    type Observers = OT;

    fn init<W: Worker>(
        &mut self,
        _state: &mut S,
        _rt_handle: &mut libaflmm::runtimes::RuntimeHandle<S, W>,
    ) -> Result<(), Error> {
        if let Some(tm) = self.timeout {
            self.set_timeout(tm);
        }
        Ok(())
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }

    fn execute<W: Worker>(
        &mut self,
        state: &mut S,
        _rt_handle: &mut libaflmm::runtimes::RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<ExitKind, Error>
    where
        S: State,
    {
        unsafe { self.execute_impl(state, input) }
    }

    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind, Error> {
        let context = state.context_mut();
        let bytes = context.to_bytes(input);

        state.increment_execs();

        let buffer = bytes.as_slice();

        if buffer.len() > self.helper.nyx_process.input_buffer_size() {
            return Err(Error::illegal_state(format!(
                "Input does not fit in the Nyx input buffer.\
                You may want to increase the Nyx input buffer size: {} > {}",
                buffer.len(),
                self.helper.nyx_process.input_buffer_size()
            )));
        }

        self.helper
            .nyx_stdout
            .set_len(0)
            .map_err(|e| Error::illegal_state(format!("Failed to clear Nyx stdout: {e}")))?;
        self.helper.nyx_stdout.rewind()?;

        let size = u32::try_from(buffer.len()).map_err(|err| {
            Error::unsupported(format!(
                "Inputs larger than 4GB are not supported. Tried {} bytes ({err:?})",
                buffer.len()
            ))
        })?;

        // `QemuProcess::set_hprintf_fd` assumes ownership of the passed file descriptor, so we
        // duplicate `self.helper.nyx_stdout`'s fd here to prevent it from being closed by
        // `QemuProcess`.
        //
        // Note: we use libc directly since `nix::unistd::dup` returns an `OwnedFd` which would
        // lead to a double close scenario when the `OwnedFd` is dropped and `set_hprintf_fd` is
        // called later on.
        //
        // # Safety
        let hprintf_fd = unsafe { nix::libc::dup(self.helper.nyx_stdout.as_raw_fd()) };

        self.helper.nyx_process.set_input(buffer, size);
        self.helper.nyx_process.set_hprintf_fd(hprintf_fd);

        unsafe {
            if CMPLOG_ENABLED == 1 {
                self.helper.nyx_process.option_set_redqueen_mode(true);
                self.helper.nyx_process.option_apply();
            }
        }

        // exec will take care of trace_bits, so no need to reset
        let exit_kind = match self.helper.nyx_process.exec() {
            NyxReturnValue::Normal => ExitKind::Ok,
            NyxReturnValue::Crash | NyxReturnValue::Asan => ExitKind::Crash,
            NyxReturnValue::Timeout => ExitKind::Timeout,
            NyxReturnValue::InvalidWriteToPayload => {
                self.helper.nyx_process.shutdown();
                return Err(Error::illegal_state(
                    "FixMe: Nyx InvalidWriteToPayload handler is missing",
                ));
            }
            NyxReturnValue::Error => {
                self.helper.nyx_process.shutdown();
                return Err(Error::illegal_state("Nyx runtime error has occurred"));
            }
            NyxReturnValue::IoError => {
                self.helper.nyx_process.shutdown();
                return Err(Error::unknown("QEMU-nyx died"));
            }
            NyxReturnValue::Abort => {
                self.helper.nyx_process.shutdown();
                return Err(Error::shutting_down());
            }
        };

        if let Some(ob) = self.stdout.clone() {
            let mut stdout = Vec::new();
            self.helper.nyx_stdout.rewind()?;
            self.helper
                .nyx_stdout
                .read_to_end(&mut stdout)
                .map_err(|e| Error::illegal_state(format!("Failed to read Nyx stdout: {e}")))?;

            self.observers_mut().index_mut(&ob).observe(stdout);
        }

        unsafe {
            if CMPLOG_ENABLED == 1 {
                self.helper.nyx_process.option_set_redqueen_mode(false);
                self.helper.nyx_process.option_apply();
            }
        }

        Ok(exit_kind)
    }
}

impl<OT> NyxExecutor<OT> {
    fn set_timeout(&mut self, timeout: core::time::Duration) {
        let micros = 1000000;
        let mut timeout_secs = timeout.as_secs();
        let mut timeout_micros = timeout.as_micros() - u128::from(timeout.as_secs() * micros);
        // since timeout secs is a u8 -> convert any overflow into micro secs
        if timeout_secs > 255 {
            timeout_micros = u128::from((timeout_secs - 255) * micros);
            timeout_secs = 255;
        }

        self.helper.timeout = timeout;

        self.helper
            .set_timeout(timeout_secs as u8, timeout_micros as u32);
    }
}

impl<OT> NyxExecutor<OT> {
    /// Convert `trace_bits` ptr into real trace map
    ///
    /// # Safety
    /// Mutable borrow may only be used once at a time.
    pub unsafe fn trace_bits(self) -> &'static mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.helper.bitmap_buffer, self.helper.bitmap_size)
        }
    }
}

pub struct NyxExecutorBuilder {
    stdout: Option<Handle<StdOutObserver>>,
    timeout: Option<Duration>, // stderr: Option<StdErrObserver>,
}

impl Default for NyxExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl NyxExecutorBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stdout: None,
            timeout: None,
            // stderr: None,
        }
    }

    pub fn stdout(&mut self, stdout: Handle<StdOutObserver>) -> &mut Self {
        self.stdout = Some(stdout);
        self
    }

    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build<OT>(&self, helper: NyxHelper, observers: OT) -> NyxExecutor<OT> {
        NyxExecutor {
            helper,
            stdout: self.stdout.clone(),
            timeout: self.timeout,
            observers,
        }
    }
}
