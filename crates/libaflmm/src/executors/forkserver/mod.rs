//! Expose an `Executor` based on a `Forkserver` in order to execute AFL/AFL++ binaries

use crate::{
    Error, Result,
    common::{DependencyResolver, Registrator},
    controllers::Worker,
    executors::{Executor, ExitKind},
    inputs::InputContext,
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
    states::State,
};
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use libaflmm_bolts::{SysVShm, core_affinity::CoreId, fs::InputFile, tuples::RefIndexable};
use libaflmm_core::{
    forkserver::{
        AFL_MAP_SIZE_ENV_VAR, FS_ERROR_MAP_ADDR, FS_ERROR_MAP_SIZE, FS_ERROR_MMAP,
        FS_ERROR_OLD_CMPLOG, FS_ERROR_OLD_CMPLOG_QEMU, FS_ERROR_SHM_OPEN, FS_ERROR_SHMAT,
        SHMEM_FUZZ_HDR_SIZE,
    },
    runtime, unknown,
};
use nix::{
    sys::{
        signal::{Signal, kill},
        time::TimeSpec,
    },
    unistd::Pid,
};
use static_assertions::const_assert_eq;
use std::{ffi::OsString, os::unix::io::RawFd, path::PathBuf};

pub mod builder;
pub use builder::ForkserverExecutorBuilder;

pub mod channel;
pub use channel::ForkserverChannel;

pub mod config;
pub(crate) use config::Config;

type ForkserverShmSize = u32;
type ForkserverShm = SysVShm<ForkserverShmSize>;

const_assert_eq!(size_of::<ForkserverShmSize>(), SHMEM_FUZZ_HDR_SIZE);

/// The default signal to use to kill child processes
const KILL_SIGNAL_DEFAULT: Signal = Signal::SIGTERM;

#[expect(clippy::struct_excessive_bools)]
pub struct ForkserverConfig {
    target: OsString,
    args: Vec<OsString>,
    envs: Vec<(OsString, OsString)>,
    input_filefd: RawFd,
    use_stdin: bool,
    memlimit: u64,
    is_persistent: bool,
    is_deferred_frksrv: bool,
    is_fsrv_only: bool,
    coverage_map_size: Option<usize>,
    debug_output: bool,
    kill_signal: Signal,
    stdout_memfd: Option<RawFd>,
    stderr_memfd: Option<RawFd>,
    cwd: Option<PathBuf>,
    core: Option<CoreId>,
}

/// This [`Executor`] can run binaries compiled for AFL/AFL++ that make use of a forkserver.
///
/// Shared memory feature is also available, but you have to set things up in your code.
/// Please refer to AFL++'s docs. <https://github.com/AFLplusplus/AFLplusplus/blob/stable/instrumentation/README.persistent_mode.md>
pub struct ForkserverExecutor<OT> {
    observers: OT,
    forkserver: ForkserverChannel,
    input_file: InputFile,
    map: Option<ForkserverShm>,
    target: OsString,
    args: Vec<OsString>,
    uses_shmem_testcase: bool,
    min_input_size: usize,
    max_input_size: usize,
    timeout: TimeSpec,
    crash_exitcode: Option<i8>,
}

#[allow(non_snake_case)]
fn report_error_and_exit(status: i32) -> Result<()> {
    /* Report on the error received via the forkserver controller and exit */
    match status {
        FS_ERROR_MAP_SIZE => Err(Error::unknown(format!(
            "{AFL_MAP_SIZE_ENV_VAR} is not set and fuzzing target reports that the required size is very large. Solution: Run the fuzzing target stand-alone with the environment variable AFL_DEBUG=1 set and set the value for __afl_final_loc in the {AFL_MAP_SIZE_ENV_VAR} environment variable for afl-fuzz."
        ))),
        FS_ERROR_MAP_ADDR => Err(Error::unknown(
            "the fuzzing target reports that hardcoded map address might be the reason the mmap of the shared memory failed. Solution: recompile the target with either afl-clang-lto and do not set AFL_LLVM_MAP_ADDR or recompile with afl-clang-fast.",
        )),
        FS_ERROR_SHM_OPEN => Err(Error::unknown(
            "the fuzzing target reports that the shm_open() call failed.",
        )),
        FS_ERROR_SHMAT => Err(Error::unknown(
            "the fuzzing target reports that the shmat() call failed.",
        )),
        FS_ERROR_MMAP => Err(Error::unknown(
            "the fuzzing target reports that the mmap() call to the shared memory failed.",
        )),
        FS_ERROR_OLD_CMPLOG => Err(Error::unknown(
            "the -c cmplog target was instrumented with an too old AFL++ version, you need to recompile it.",
        )),
        FS_ERROR_OLD_CMPLOG_QEMU => Err(Error::unknown(
            "The AFL++ QEMU/FRIDA loaders are from an older version, for -c you need to recompile it.",
        )),
        _ => Err(unknown!("unknown error code {status} from fuzzing target!")),
    }
}

impl<OT> Debug for ForkserverExecutor<OT>
where
    OT: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForkserverExecutor")
            .field("target", &self.target)
            .field("args", &self.args)
            .field("input_file", &self.input_file)
            .field("uses_shmem_testcase", &self.uses_shmem_testcase)
            .field("forkserver", &self.forkserver)
            .field("observers", &self.observers)
            .field("map", &self.map)
            .finish_non_exhaustive()
    }
}

impl ForkserverExecutor<()> {
    /// Builder for `ForkserverExecutor`
    #[must_use]
    pub fn builder() -> ForkserverExecutorBuilder<'static> {
        ForkserverExecutorBuilder::new()
    }
}

impl<OT> ForkserverExecutor<OT> {
    fn map_input_to_shmem(&mut self, input: &[u8], input_size: usize) -> Result<()> {
        if self.uses_shmem_testcase {
            debug_assert!(
                self.map.is_some(),
                "The uses_shmem_testcase() bool can only exist when a map is set"
            );

            unsafe {
                self.map
                    .as_mut()
                    .unwrap_unchecked()
                    .shm_mut()
                    .write(&input[..input_size])?;
            }
        } else {
            self.input_file.write_buf(&input[..input_size])?;
        }
        Ok(())
    }

    /// Execute input, but side-step the execution counter.
    #[inline]
    fn execute_input(&mut self, input: &[u8]) -> Result<ExitKind> {
        let mut exit_kind = ExitKind::Ok;
        let last_run_timed_out = self.forkserver.last_run_timed_out_raw();

        let mut input_size = input.len();
        if input_size > self.max_input_size {
            // Truncate like AFL++ does
            input_size = self.max_input_size;
            self.map_input_to_shmem(input, input_size)?;
        } else if input_size < self.min_input_size {
            // Extend like AFL++ does: copy input then zero-pad to min_input_size
            let mut padded = input.to_vec();
            padded.resize(self.min_input_size, 0);
            self.map_input_to_shmem(&padded, self.min_input_size)?;
        } else {
            self.map_input_to_shmem(input, input_size)?;
        }

        self.forkserver.set_last_run_timed_out(false);
        if let Err(err) = self.forkserver.write_ctl(last_run_timed_out) {
            return Err(runtime!(
                "Unable to request new process from fork server (OOM?): {err:?}"
            ));
        }

        let pid = self.forkserver.read_st().map_err(|err| {
            runtime!("Unable to request new process from fork server (OOM?): {err:?}")
        })?;

        if pid <= 0 {
            return Err(runtime!("Fork server is misbehaving (OOM?)"));
        }

        self.forkserver.set_child_pid(Pid::from_raw(pid));

        let timeout = self.timeout;
        if let Some(status) = self.forkserver.read_st_timeout(&timeout)? {
            self.forkserver.set_status(status);
            let exitcode_is_crash = if let Some(crash_exitcode) = self.crash_exitcode {
                (libc::WEXITSTATUS(self.forkserver.status()) as i8) == crash_exitcode
            } else {
                false
            };
            if libc::WIFSIGNALED(self.forkserver.status()) || exitcode_is_crash {
                exit_kind = ExitKind::Crash;
            }
        } else {
            self.forkserver.set_last_run_timed_out(true);

            // We need to kill the child in case he has timed out, or we can't get the correct pid in the next call to self.executor.forkserver_mut().read_st()?
            if let Some(pid) = self.forkserver.child_pid() {
                let _ = kill(pid, self.forkserver.kill_signal());
            }
            if let Err(err) = self.forkserver.read_st() {
                return Err(runtime!("Could not kill timed-out child: {err:?}"));
            }
            exit_kind = ExitKind::Timeout;
        }

        if !libc::WIFSTOPPED(self.forkserver.status()) {
            self.forkserver.reset_child_pid();
        }

        Ok(exit_kind)
    }
}

impl<I, OT, S> Executor<I, S> for ForkserverExecutor<OT>
where
    OT: ObserversTuple<S> + DependencyResolver,
    S: State<Input = I>,
{
    type Observers = OT;

    fn init<W: Worker>(
        &mut self,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }

    fn execute<W: Worker>(
        &mut self,
        state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<ExitKind> {
        state.increment_execs();

        self.observers_mut().pre_exec_all(state)?;

        let exit_kind = unsafe { self.execute_impl(state, input)? };

        self.observers_mut()
            .post_exec_all(state, &exit_kind)
            .map(|()| exit_kind)
    }

    #[inline]
    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind> {
        let context = state.context_mut();
        let bytes = context.to_bytes(input);
        let exit = self.execute_input(&bytes)?;
        Ok(exit)
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}

impl<OT> DependencyResolver for ForkserverExecutor<OT>
where
    OT: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_impl(registrator)?;

        self.observers.register(registrator)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use libaflmm_bolts::{AsSliceMut, StdTargetArgs, SysVShm, tuples::tuple_list};
    use serial_test::serial;

    use crate::{
        Error,
        executors::{
            StdChildArgs, builder::FAILED_TO_START_FORKSERVER_MSG, forkserver::ForkserverExecutor,
        },
        observers::{ConstMapObserver, HitcountsMapObserver},
        runtimes::RuntimeHandle,
    };

    #[test]
    #[serial]
    #[cfg_attr(miri, ignore)]
    #[cfg_attr(target_pointer_width = "32", ignore)] // TODO: Why does this fail?
    fn test_forkserver() {
        const MAP_SIZE: usize = 65536;
        let bin = OsString::from("echo");
        let args = vec![OsString::from("@@")];

        let mut shmem = SysVShm::new(MAP_SIZE).unwrap();
        // # Safety
        // There's a slight chance this is racey but very unlikely in the normal use case
        unsafe {
            shmem.write_to_env("__AFL_SHM_ID").unwrap();
        }
        let shmem_buf: &mut [u8; MAP_SIZE] = shmem.as_slice_mut().try_into().unwrap();

        let edges_observer = HitcountsMapObserver::new(ConstMapObserver::<_, MAP_SIZE>::new(
            "shared_mem",
            shmem_buf,
        ));

        let rt_handle = unsafe { RuntimeHandle::empty() };

        let executor = ForkserverExecutor::builder()
            .program(bin)
            .args(args)
            .coverage_map_size(MAP_SIZE)
            .debug_child(false)
            .build(tuple_list!(edges_observer), &rt_handle);

        // Since /usr/bin/echo is not a instrumented binary file, the test will just check if the forkserver has failed at the initial handshake
        let result = match executor {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Error: {e:?}");
                match e {
                    Error::IllegalState(s, _) => s.contains(FAILED_TO_START_FORKSERVER_MSG),
                    _ => false,
                }
            }
        };
        assert!(result);
    }
}
