//! Executors take input, and run it in the target.

use crate::observers::{StdErrObserver, StdOutObserver};
use crate::{
    Result,
    common::DependencyResolver,
    controllers::Worker,
    observers::ObserversTuple,
    runtimes::{
        RuntimeHandle,
        inprocess::{CrashStatus, TimeoutStatus},
        utils::unix::signal::OsTerminationParams,
    },
    states::State,
};
use ::std::path::PathBuf;
use alloc::vec::Vec;
use core::{fmt::Debug, time::Duration};
#[cfg(unix)]
use libaflmm_bolts::exceptions::unix_signals::Signal;
use libaflmm_bolts::tuples::RefIndexable;
use libaflmm_bolts::{core_affinity::CoreId, tuples::Handle};
use serde::{Deserialize, Serialize};

pub mod hooks;
pub use hooks::{ExecutorHook, ExecutorHooksTuple};

pub mod nop;
pub use nop::NopExecutor;

#[cfg(unix)]
pub mod forkserver;
#[cfg(unix)]
pub use forkserver::*;

pub mod standard;
pub use standard::StdExecutor;

/// How an execution finished.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub enum ExitKind {
    /// The run exited normally.
    Ok,
    /// The run resulted in a target crash.
    Crash,
    /// The run hit an out of memory error.
    Oom,
    /// The run timed out
    Timeout,
}
libaflmm_bolts::impl_serdeany!(ExitKind);

/// How one of the diffing executions finished.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub enum DiffExitKind {
    /// The run exited normally.
    Ok,
    /// The run resulted in a target crash.
    Crash,
    /// The run hit an out of memory error.
    Oom,
    /// The run timed out
    Timeout,
    /// One of the executors itelf repots a differential, we can't go into further details.
    Diff,
    // The run resulted in a custom `ExitKind`.
    // Custom(Box<dyn SerdeAny>),
}
libaflmm_bolts::impl_serdeany!(DiffExitKind);

/// Runs the fuzzer harness.
pub trait Executor<I, S>: DependencyResolver {
    /// The [`Observer`](crate::observers::Observer)s owned by the [`Executor`].
    type Observers: ObserversTuple<S>;

    /// The init function of the executor.
    /// It must be run once before the first execution of the executor.
    fn init<W: Worker>(&mut self, state: &mut S, rt_handle: &mut RuntimeHandle<S, W>)
    -> Result<()>;

    /// Run the target with the given input.
    /// This is a "raw" run: it only runs the target and nothing else is done.
    /// More particularly:
    ///     - observers are untouched
    ///     - state is not updated
    ///     - timeout is not re-armed
    ///
    /// You most likely do NOT want to use this function directly, except if calling
    /// in an inner executor.
    /// Prefer `run_target` in most cases.
    ///
    ///
    /// # Safety
    ///
    /// This function is subject to timeouts, and signals can be raised asynchronously from this point onwards.
    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind>;

    /// Run the target with the given input.
    /// State and observers are updated accordingly.
    ///
    /// This is the main function to run an input.
    fn execute<W: Worker>(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<ExitKind>
    where
        S: State,
    {
        state.increment_execs();

        // start_timer!(state);
        self.observers_mut().pre_exec_all(state)?;
        // mark_feature_time!(state, PerfFeature::PreExecObservers);

        rt_handle.arm_timeout()?;

        // start_timer!(state);
        rt_handle.set_input(input);
        let exit_kind = unsafe { self.execute_impl(state, input)? };
        rt_handle.clear_input();
        // mark_feature_time!(state, PerfFeature::TargetExecution);

        rt_handle.disarm_timeout()?;

        // start_timer!(state);
        self.observers_mut()
            .post_exec_all(state, &exit_kind)
            .map(|()| exit_kind)
        // mark_feature_time!(state, PerfFeature::PostExecObservers);
    }

    /// Get the linked observers
    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers>;

    /// Get the linked observers (mutable)
    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers>;

    /// Handle crash.
    ///
    /// Returns whether the crash is due to the fuzzer or the target.
    /// By default, it returns [`CrashStatus::TargetCrash`].
    ///
    /// # Safety
    ///
    /// This will run in a signal handler, so it's very constrained.
    /// In particular, it should not allocate anything in the heap.
    unsafe fn handle_crash(
        &mut self,
        _state: &mut S,
        _input: Option<&I>,
        _params: &OsTerminationParams,
    ) -> Result<CrashStatus> {
        Ok(CrashStatus::TargetCrash)
    }

    /// Handle timeout
    ///
    /// Returns what the fuzzer should do on timeout.
    /// By default, it returns [`TimeoutStatus::Exit`].
    ///
    /// # Safety
    ///
    /// This will run in a signal handler, so it's very constrained.
    /// In particular, it should not allocate anything in the heap.
    unsafe fn handle_timeout(
        &mut self,
        _state: &mut S,
        _input: Option<&I>,
        _params: &OsTerminationParams,
    ) -> Result<TimeoutStatus> {
        Ok(TimeoutStatus::Exit)
    }
}

/// Like [`crate::observers::ObserversTuple`], a list of executors
pub trait ExecutorsTuple<EM, I, S, Z> {
    /// Execute the executors and stop if any of them returns a crash
    fn run_target_all(
        &mut self,
        fuzzer: &mut Z,
        state: &mut S,
        mgr: &mut EM,
        input: &I,
    ) -> Result<ExitKind>;
}

/// The common signals we want to handle
#[cfg(unix)]
#[inline]
#[must_use]
pub fn common_signals() -> Vec<Signal> {
    vec![
        Signal::SigAlarm,
        Signal::SigUser2,
        Signal::SigAbort,
        Signal::SigBus,
        #[cfg(feature = "handle_sigpipe")]
        Signal::SigPipe,
        Signal::SigFloatingPointException,
        Signal::SigIllegalInstruction,
        Signal::SigSegmentationFault,
        Signal::SigTrap,
    ]
}

/// The inner shared members of [`StdChildArgs`]
#[derive(Debug, Clone)]
pub struct StdChildArgsInner {
    /// The timeout of the children
    pub timeout: Duration,
    /// The stderr handle of the children
    pub stderr_observer: Option<Handle<StdErrObserver>>,
    /// The stdout handle of the children
    pub stdout_observer: Option<Handle<StdOutObserver>>,
    /// The current directory of the spawned children
    pub current_directory: Option<PathBuf>,
    /// Whether debug child by inheriting stdout/stderr
    pub debug_child: bool,
    /// Core to bind for the children
    pub core: Option<CoreId>,
}

impl Default for StdChildArgsInner {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            stderr_observer: None,
            stdout_observer: None,
            current_directory: None,
            debug_child: false,
            core: None,
        }
    }
}

/// The shared implementation for children with stdout/stderr/timeouts.
pub trait StdChildArgs: Sized {
    /// The inner struct of child environment.
    fn inner(&self) -> &StdChildArgsInner;

    /// The mutable inner struct of child environment.
    fn inner_mut(&mut self) -> &mut StdChildArgsInner;

    #[must_use]
    /// Sets the execution timeout duration.
    fn timeout(mut self, timeout: Duration) -> Self {
        self.inner_mut().timeout = timeout;
        self
    }

    #[must_use]
    /// Sets the stdout observer
    fn stdout_observer(mut self, stdout: Handle<StdOutObserver>) -> Self {
        self.inner_mut().stdout_observer = Some(stdout);
        self
    }

    #[must_use]
    /// Sets the stderr observer
    fn stderr_observer(mut self, stderr: Handle<StdErrObserver>) -> Self {
        self.inner_mut().stderr_observer = Some(stderr);
        self
    }

    #[must_use]
    /// Sets the working directory for the child process.
    fn current_dir(mut self, current_dir: PathBuf) -> Self {
        self.inner_mut().current_directory = Some(current_dir);
        self
    }

    #[must_use]
    /// If set to true, the child's output won't be redirecited to `/dev/null` and will go to parent's stdout/stderr
    /// Defaults to `false`.
    fn debug_child(mut self, debug_child: bool) -> Self {
        if debug_child {
            assert!(
                self.inner().stderr_observer.is_none() && self.inner().stdout_observer.is_none(),
                "you can not set debug_child when you have stderr_observer or stdout_observer"
            );
        }
        self.inner_mut().debug_child = debug_child;
        self
    }

    #[must_use]
    /// Set the core to bind for the children
    fn core(mut self, core: CoreId) -> Self {
        self.inner_mut().core = Some(core);
        self
    }
}
