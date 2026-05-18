//! Expose an `Executor` based on a `Forkserver` in order to execute AFL/AFL++ binaries

use alloc::{string::ToString, vec::Vec};
use core::{
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
};
use std::{
    env,
    ffi::OsString,
    io::{self, ErrorKind, Read, Write},
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::io::RawFd,
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use libaflmm_bolts::{
    InputLocation, Pipe, StdTargetArgs, StdTargetArgsInner, SysVShm, Truncate,
    core_affinity::CoreId,
    fs::{InputFile, get_unique_std_input_file},
    tuples::{MatchNameRef, RefIndexable},
};
use libaflmm_core::forkserver::{
    AFL_GCC_ONLY_FSRV_VAR, AFL_LLVM_ONLY_FSRV_VAR, AFL_MAP_SIZE_ENV_VAR, FS_ERROR_MAP_ADDR,
    FS_ERROR_MAP_SIZE, FS_ERROR_MMAP, FS_ERROR_OLD_CMPLOG, FS_ERROR_OLD_CMPLOG_QEMU,
    FS_ERROR_SHM_OPEN, FS_ERROR_SHMAT, FS_NEW_ERROR, FS_NEW_OPT_AUTODTCT, FS_NEW_OPT_MAPSIZE,
    FS_NEW_OPT_SHDMEM_FUZZ, FS_NEW_VERSION_MAGIC_BASE, FS_NEW_VERSION_MAX, FS_NEW_VERSION_MIN,
    MAX_INPUT_SIZE_DEFAULT, MIN_INPUT_SIZE_DEFAULT, SHM_ENV_VAR, SHM_FUZZ_ENV_VAR,
    SHM_FUZZ_MAP_SIZE_ENV_VAR, SHMEM_FUZZ_HDR_SIZE,
};
use nix::{
    sys::{
        select::{FdSet, pselect},
        signal::{SigSet, Signal, kill},
        time::TimeSpec,
        wait::waitpid,
    },
    unistd::Pid,
};
use static_assertions::const_assert_eq;

use super::{StdChildArgs, StdChildArgsInner};
use crate::{
    DependencyResolver, Error, Result,
    executors::{Executor, ExitKind},
    inputs::InputContext,
    mutators::Tokens,
    observers::{MapObserver, ObserversTuple},
    runtimes::RuntimeHandle,
    states::State,
};

pub mod config;
pub(crate) use config::Config;

type ForkserverShmSize = u32;
type ForkserverShm = SysVShm<ForkserverShmSize>;

const_assert_eq!(size_of::<ForkserverShmSize>(), SHMEM_FUZZ_HDR_SIZE);

/// Forkserver message. We'll reuse it in a testcase.
const FAILED_TO_START_FORKSERVER_MSG: &str = "Failed to start forkserver";

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
        _ => Err(Error::unknown(format!(
            "unknown error code {status} from fuzzing target!"
        ))),
    }
}

/// The default signal to use to kill child processes
const KILL_SIGNAL_DEFAULT: Signal = Signal::SIGTERM;

/// The [`Forkserver`] is communication channel with a child process that forks on request of the fuzzer.
/// The communication happens via pipe.
#[derive(Debug)]
pub struct Forkserver {
    /// The "actual" forkserver we spawned in the target
    fsrv_handle: Child,
    /// Status pipe
    st_pipe: Pipe,
    /// Control pipe
    ctl_pipe: Pipe,
    /// Pid of the current forked child (child of the forkserver) during execution
    child_pid: Option<Pid>,
    /// The last status reported to us by the in-target forkserver
    status: i32,
    /// If the last run timed out (in in-target i32)
    last_run_timed_out: i32,
    /// The signal this [`Forkserver`] will use to kill
    kill_signal: Signal,
}

impl Drop for Forkserver {
    fn drop(&mut self) {
        // Modelled after <https://github.com/AFLplusplus/AFLplusplus/blob/dee76993812fa9b5d8c1b75126129887a10befae/src/afl-forkserver.c#L1429>
        log::debug!("Dropping forkserver");

        if let Some(pid) = self.child_pid {
            log::debug!("Sending {} to child {pid}", self.kill_signal);
            if let Err(err) = kill(pid, self.kill_signal) {
                log::warn!(
                    "Failed to deliver kill signal to child process {}: {err} ({})",
                    pid,
                    io::Error::last_os_error()
                );
            }
        }

        let forkserver_pid = Pid::from_raw(self.fsrv_handle.id().try_into().unwrap());
        if let Err(err) = kill(forkserver_pid, self.kill_signal) {
            log::warn!(
                "Failed to deliver {} signal to forkserver {}: {err} ({})",
                self.kill_signal,
                forkserver_pid,
                io::Error::last_os_error()
            );
            let _ = kill(forkserver_pid, Signal::SIGKILL);
        } else if let Err(err) = waitpid(forkserver_pid, None) {
            log::warn!(
                "Waitpid on forkserver {} failed: {err} ({})",
                forkserver_pid,
                io::Error::last_os_error()
            );
            let _ = kill(forkserver_pid, Signal::SIGKILL);
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
struct ForkserverSpawnConfig {
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

#[allow(unstable_name_collisions)]
impl Forkserver {
    /// Create a new [`Forkserver`] that will kill child processes
    /// with the given `kill_signal`.
    /// Using `Forkserver::new(..)` will default to [`Signal::SIGTERM`].
    fn new(cfg: ForkserverSpawnConfig) -> Result<Self> {
        let ForkserverSpawnConfig {
            target,
            args,
            envs,
            input_filefd,
            use_stdin,
            memlimit,
            is_persistent,
            is_deferred_frksrv,
            is_fsrv_only,
            coverage_map_size,
            debug_output,
            kill_signal,
            stdout_memfd,
            stderr_memfd,
            cwd,
            core,
        } = cfg;
        let Some(coverage_map_size) = coverage_map_size else {
            return Err(Error::unknown(
                "Coverage map size unknown. Use coverage_map_size() to tell the forkserver about the map size.",
            ));
        };

        if env::var(SHM_ENV_VAR).is_err() {
            return Err(Error::unknown(
                "__AFL_SHM_ID not set. It is necessary to set this env, otherwise the forkserver cannot communicate with the fuzzer",
            ));
        }

        let afl_debug = if let Ok(afl_debug) = env::var("AFL_DEBUG") {
            if afl_debug != "1" && afl_debug != "0" {
                return Err(Error::illegal_argument("AFL_DEBUG must be either 1 or 0"));
            }
            afl_debug == "1"
        } else {
            false
        };

        let mut st_pipe = Pipe::new().unwrap();
        let mut ctl_pipe = Pipe::new().unwrap();

        let mut command = Command::new(target);
        // Setup args, stdio
        command.args(args);
        if use_stdin {
            // # Safety
            // We assume the file descriptors will be valid and not closed.
            unsafe {
                command.setdup2(input_filefd, libc::STDIN_FILENO);
            }
        } else {
            command.stdin(Stdio::null());
        }

        if debug_output {
            command.stdout(Stdio::inherit());
        } else if let Some(fd) = &stdout_memfd {
            // # Safety
            // We assume the file descriptors will be valid and not closed.
            unsafe {
                command.setdup2(*fd, libc::STDOUT_FILENO);
            }
            command.stdout(Stdio::null());
        } else {
            command.stdout(Stdio::null());
        }

        if debug_output {
            command.stderr(Stdio::inherit());
        } else if let Some(fd) = &stderr_memfd {
            // # Safety
            // We assume the file descriptors will be valid and not closed.
            unsafe {
                command.setdup2(*fd, libc::STDERR_FILENO);
            }
            command.stderr(Stdio::null());
        } else {
            command.stderr(Stdio::null());
        }

        if let Some(core) = core {
            command.bind(core);
        }

        command.env(AFL_MAP_SIZE_ENV_VAR, format!("{coverage_map_size}"));

        // Persistent, deferred forkserver
        if is_persistent {
            command.env("__AFL_PERSISTENT", "1");
        }

        if is_deferred_frksrv {
            command.env("__AFL_DEFER_FORKSRV", "1");
        }

        if is_fsrv_only {
            command.env(AFL_GCC_ONLY_FSRV_VAR, "1");
            command.env(AFL_LLVM_ONLY_FSRV_VAR, "1");
        }

        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }

        // # Saftey
        // The pipe file descriptors used for `setpipe` are valid at this point.
        let fsrv_handle = unsafe {
            match Config::setsid(
                command
                    .env("LD_BIND_NOW", "1")
                    .envs(envs)
                    .setlimit(memlimit)
                    .set_coredump(afl_debug),
            )
            .setpipe(st_pipe.write_end().unwrap(), ctl_pipe.read_end().unwrap())
            .spawn()
            {
                Ok(fsrv_handle) => fsrv_handle,
                Err(err) => {
                    return Err(Error::illegal_state(format!(
                        "Could not spawn the forkserver: {err:#?}"
                    )));
                }
            }
        };

        // Ctl_pipe.read_end and st_pipe.write_end are unnecessary for the parent, so we'll close them
        ctl_pipe.close_read_end();
        st_pipe.close_write_end();

        Ok(Self {
            fsrv_handle,
            st_pipe,
            ctl_pipe,
            child_pid: None,
            status: 0,
            last_run_timed_out: 0,
            kill_signal,
        })
    }

    /// If the last run timed out (as in-target i32)
    #[must_use]
    pub fn last_run_timed_out_raw(&self) -> i32 {
        self.last_run_timed_out
    }

    /// If the last run timed out
    #[must_use]
    pub fn last_run_timed_out(&self) -> bool {
        self.last_run_timed_out_raw() != 0
    }

    /// Sets if the last run timed out (as in-target i32)
    #[inline]
    pub fn set_last_run_timed_out_raw(&mut self, last_run_timed_out: i32) {
        self.last_run_timed_out = last_run_timed_out;
    }

    /// Sets if the last run timed out
    #[inline]
    pub fn set_last_run_timed_out(&mut self, last_run_timed_out: bool) {
        self.last_run_timed_out = i32::from(last_run_timed_out);
    }

    /// The status
    #[must_use]
    pub fn status(&self) -> i32 {
        self.status
    }

    /// Sets the status
    pub fn set_status(&mut self, status: i32) {
        self.status = status;
    }

    /// The child pid
    #[must_use]
    pub fn child_pid(&self) -> Option<Pid> {
        self.child_pid
    }

    /// Set the child pid
    pub fn set_child_pid(&mut self, child_pid: Pid) {
        self.child_pid = Some(child_pid);
    }

    /// Remove the child pid.
    pub fn reset_child_pid(&mut self) {
        self.child_pid = None;
    }

    /// Read from the st pipe
    pub fn read_st(&mut self) -> Result<i32> {
        let mut buf: [u8; 4] = [0_u8; 4];
        let rlen = self.st_pipe.read(&mut buf)?;
        if rlen == size_of::<i32>() {
            Ok(i32::from_ne_bytes(buf))
        } else {
            // NOTE: The underlying API does not guarantee that the read will return
            //       exactly four bytes, but the chance of this happening is very low.
            //       This is a sacrifice of correctness for performance.
            Err(Error::illegal_state(format!(
                "Could not read from st pipe. Expected {} bytes, got {rlen} bytes",
                size_of::<i32>()
            )))
        }
    }

    /// Read bytes of any length from the st pipe
    pub fn read_st_of_len(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        self.st_pipe.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Write to the ctl pipe
    pub fn write_ctl(&mut self, val: i32) -> Result<()> {
        let slen = self.ctl_pipe.write(&val.to_ne_bytes())?;
        if slen == size_of::<i32>() {
            Ok(())
        } else {
            // NOTE: The underlying API does not guarantee that exactly four bytes
            //       are written, but the chance of this happening is very low.
            //       This is a sacrifice of correctness for performance.
            Err(Error::illegal_state(format!(
                "Could not write to ctl pipe. Expected {} bytes, wrote {slen} bytes",
                size_of::<i32>()
            )))
        }
    }

    /// Read a message from the child process.
    pub fn read_st_timed(&mut self, timeout: &TimeSpec) -> Result<Option<i32>> {
        let mut buf: [u8; 4] = [0_u8; 4];
        let Some(st_read) = self.st_pipe.read_end() else {
            return Err(Error::os_error(
                io::Error::new(ErrorKind::BrokenPipe, "Read pipe end was already closed"),
                "read_st_timed failed",
            ));
        };

        // # Safety
        // The FDs are valid as this point in time.
        let st_read = unsafe { BorrowedFd::borrow_raw(st_read) };

        let mut readfds = FdSet::new();
        readfds.insert(st_read);
        // We'll pass a copied timeout to keep the original timeout intact, because select updates timeout to indicate how much time was left. See select(2)
        let sret = pselect(
            Some(readfds.highest().unwrap().as_raw_fd() + 1),
            &mut readfds,
            None,
            None,
            Some(timeout),
            Some(&SigSet::empty()),
        )?;
        if sret > 0 {
            if self.st_pipe.read_exact(&mut buf).is_ok() {
                let val: i32 = i32::from_ne_bytes(buf);
                Ok(Some(val))
            } else {
                Err(Error::unknown(
                    "Unable to communicate with fork server (OOM?)".to_string(),
                ))
            }
        } else {
            Ok(None)
        }
    }
}

/// This [`Executor`] can run binaries compiled for AFL/AFL++ that make use of a forkserver.
///
/// Shared memory feature is also available, but you have to set things up in your code.
/// Please refer to AFL++'s docs. <https://github.com/AFLplusplus/AFLplusplus/blob/stable/instrumentation/README.persistent_mode.md>
pub struct ForkserverExecutor<OT> {
    inner: BuiltForkserver,
    observers: OT,
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

impl<OT> Deref for ForkserverExecutor<OT> {
    type Target = BuiltForkserver;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<OT> DerefMut for ForkserverExecutor<OT> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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
            return Err(Error::unknown(format!(
                "Unable to request new process from fork server (OOM?): {err:?}"
            )));
        }

        let pid = self.forkserver.read_st().map_err(|err| {
            Error::unknown(format!(
                "Unable to request new process from fork server (OOM?): {err:?}"
            ))
        })?;

        if pid <= 0 {
            return Err(Error::unknown(
                "Fork server is misbehaving (OOM?)".to_string(),
            ));
        }

        self.forkserver.set_child_pid(Pid::from_raw(pid));

        let timeout = self.timeout;
        if let Some(status) = self.forkserver.read_st_timed(&timeout)? {
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
                let _ = kill(pid, self.forkserver.kill_signal);
            }
            if let Err(err) = self.forkserver.read_st() {
                return Err(Error::unknown(format!(
                    "Could not kill timed-out child: {err:?}"
                )));
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
    S: State<I>,
{
    type Observers = OT;

    fn init<W: crate::Worker>(
        &mut self,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }

    fn execute<W: crate::Worker>(
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
    fn register_with_ty(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        registrator.register_ty::<Self>();

        self.register(registrator)?;
        self.observers.register_with_ty(registrator)
    }
}

/// The builder for `ForkserverExecutor`
#[derive(Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct ForkserverExecutorBuilder<'a> {
    target_inner: StdTargetArgsInner,
    child_env_inner: StdChildArgsInner,
    uses_shmem_testcase: bool,
    is_persistent: bool,
    is_deferred_frksrv: bool,
    is_fsrv_only: bool,
    autotokens: Option<&'a mut Tokens>,
    try_use_input_shmem: bool,
    max_input_size: usize,
    min_input_size: usize,
    map_size: Option<usize>,
    kill_signal: Option<Signal>,
    crash_exitcode: Option<i8>,
}

impl StdChildArgs for ForkserverExecutorBuilder<'_> {
    fn inner(&self) -> &StdChildArgsInner {
        &self.child_env_inner
    }

    fn inner_mut(&mut self) -> &mut StdChildArgsInner {
        &mut self.child_env_inner
    }
}

impl StdTargetArgs for ForkserverExecutorBuilder<'_> {
    fn inner(&self) -> &StdTargetArgsInner {
        &self.target_inner
    }

    fn inner_mut(&mut self) -> &mut StdTargetArgsInner {
        &mut self.target_inner
    }

    fn arg_input_arg(self) -> Self {
        panic!("ForkserverExecutor doesn't support mutating arguments")
    }
}

/// The "built" [`Forkserver`] that has completed the initial handshake and is ready to run.
#[derive(Debug)]
pub struct BuiltForkserver {
    forkserver: Forkserver,
    input_file: InputFile,
    map: Option<ForkserverShm>,
    target: OsString,
    args: Vec<OsString>,
    uses_shmem_testcase: bool,
    map_size: Option<usize>,
    min_input_size: usize,
    max_input_size: usize,
    timeout: TimeSpec,
    crash_exitcode: Option<i8>,
}

impl<'a> ForkserverExecutorBuilder<'a> {
    /// Builds [`ForkserverExecutor`].
    /// This Forkserver will attempt to provide inputs over shared mem when [`Self::try_use_input_shmem`] is used.
    /// Else this forkserver will pass the input to the target via `stdin`
    /// in case no input file is specified.
    /// If `debug_child` is set, the child will print to `stdout`/`stderr`.
    #[expect(clippy::pedantic)]
    pub fn build<OT>(mut self, observers: OT) -> Result<ForkserverExecutor<OT>>
    where
        OT: MatchNameRef,
    {
        let built = self.build_forkserver(&observers)?;
        Ok(ForkserverExecutor {
            inner: built,
            observers,
        })
    }

    /// Builds [`ForkserverExecutor`] downsizing the coverage map to fit exactly the AFL++ map size.
    #[expect(clippy::pedantic)]
    pub fn build_dynamic_map<A, MO, OT, S>(
        mut self,
        mut map_observer: A,
        other_observers: OT,
    ) -> Result<ForkserverExecutor<(A, OT)>>
    where
        A: AsMut<MO>,
        MO: MapObserver + Truncate,
        OT: ObserversTuple<S>,
        (A, OT): ObserversTuple<S>,
    {
        let built = self.build_forkserver(&other_observers)?;

        log::info!(
            "ForkserverExecutor: program: {:?}, arguments: {:?}, use_stdin: {:?}, map_size: {:?}",
            built.target,
            built.args,
            self.use_stdin(),
            built.map_size
        );

        if let Some(dynamic_map_size) = built.map_size {
            map_observer.as_mut().truncate(dynamic_map_size);
        }

        if built.uses_shmem_testcase && built.map.is_none() {
            return Err(Error::illegal_state(
                "Map must always be set for `uses_shmem_testcase`",
            ));
        }

        Ok(ForkserverExecutor {
            inner: built,
            observers: (map_observer, other_observers),
        })
    }

    #[expect(clippy::pedantic)]
    fn build_forkserver<OT>(&mut self, obs: &OT) -> Result<BuiltForkserver>
    where
        OT: MatchNameRef,
    {
        let input_file = match &self.target_inner.input_location {
            InputLocation::StdIn {
                input_file: out_file,
            } => match out_file {
                Some(out_file) => out_file.clone(),
                None => InputFile::create(OsString::from(get_unique_std_input_file()))?,
            },
            InputLocation::Arg { argnum: _ } => {
                return Err(Error::illegal_argument(
                    "forkserver doesn't support argument mutation",
                ));
            }
            InputLocation::File { out_file } => out_file.clone(),
        };

        let map = if self.try_use_input_shmem {
            let mut shmem: ForkserverShm = SysVShm::new_with_hdr(self.max_input_size)?;

            unsafe {
                shmem.write_to_env(SHM_FUZZ_ENV_VAR)?;
                env::set_var(SHM_FUZZ_MAP_SIZE_ENV_VAR, format!("{}", shmem.total_len()));
            }

            unsafe {
                shmem
                    .shm_mut()
                    .set_size(self.max_input_size + SHMEM_FUZZ_HDR_SIZE);
            }

            Some(shmem)
        } else {
            None
        };

        let target = self.target_inner.program.take().ok_or_else(|| {
            Error::illegal_argument("ForkserverExecutorBuilder::build: target file not found")
        })?;

        let mut forkserver = Forkserver::new(ForkserverSpawnConfig {
            target: target.clone(),
            args: self.target_inner.arguments.clone(),
            envs: self.target_inner.envs.clone(),
            input_filefd: input_file.as_raw_fd(),
            use_stdin: self.use_stdin(),
            memlimit: 0,
            is_persistent: self.is_persistent,
            is_deferred_frksrv: self.is_deferred_frksrv,
            is_fsrv_only: self.is_fsrv_only,
            coverage_map_size: self.map_size,
            debug_output: self.child_env_inner.debug_child,
            kill_signal: self.kill_signal.unwrap_or(KILL_SIGNAL_DEFAULT),
            stdout_memfd: self.child_env_inner.stdout_observer.as_ref().map(|t| {
                obs.get(t)
                    .as_ref()
                    .expect("stdout observer not passed in the builder")
                    .as_raw_fd()
                    .expect("only memory fd backend is allowed for forkserver executor")
            }),
            stderr_memfd: self.child_env_inner.stderr_observer.as_ref().map(|t| {
                obs.get(t)
                    .as_ref()
                    .expect("stderr observer not passed in the builder")
                    .as_raw_fd()
                    .expect("only memory fd backend is allowed for forkserver executor")
            }),
            cwd: self.child_env_inner.current_directory.clone(),
            core: self.child_env_inner.core,
        })?;

        // Initial handshake, read 4-bytes hello message from the forkserver.
        let version_status = forkserver.read_st().map_err(|err| {
            Error::illegal_state(format!("{FAILED_TO_START_FORKSERVER_MSG}: {err:?}"))
        })?;

        if (version_status & FS_NEW_ERROR) == FS_NEW_ERROR {
            report_error_and_exit(version_status & 0x0000ffff)?;
        }

        self.initialize_forkserver(version_status, map.is_some(), &mut forkserver)?;

        if self.uses_shmem_testcase && map.is_none() {
            return Err(Error::illegal_state(
                "Map must always be set for `uses_shmem_testcase`",
            ));
        }

        log::info!(
            "ForkserverExecutor: program: {:?}, arguments: {:?}, use_stdin: {:?}, map_size: {:?}",
            target,
            self.target_inner.arguments,
            self.use_stdin(),
            self.map_size
        );

        Ok(BuiltForkserver {
            forkserver,
            input_file,
            map,
            target,
            args: self.target_inner.arguments.clone(),
            uses_shmem_testcase: self.uses_shmem_testcase,
            map_size: self.map_size,
            min_input_size: self.min_input_size,
            max_input_size: self.max_input_size,
            timeout: self.child_env_inner.timeout.into(),
            crash_exitcode: self.crash_exitcode,
        })
    }

    /// Intialize forkserver > v4.20c
    #[expect(clippy::cast_possible_wrap)]
    #[expect(clippy::cast_sign_loss)]
    fn initialize_forkserver(
        &mut self,
        status: i32,
        input_map_allocated: bool,
        forkserver: &mut Forkserver,
    ) -> Result<()> {
        let initial_status = status;
        let version: u32 = status as u32 - FS_NEW_VERSION_MAGIC_BASE;
        match version {
            0 => {
                return Err(Error::illegal_state(
                    "Fork server version is not assigned, this should not happen. Recompile target.",
                ));
            }
            FS_NEW_VERSION_MIN..=FS_NEW_VERSION_MAX => {
                // good, do nothing
            }
            _ => {
                return Err(Error::illegal_state(
                    "Fork server version is not supported. Recompile the target.",
                ));
            }
        }

        let xored_status = (status as u32 ^ 0xffffffff) as i32;

        if let Err(err) = forkserver.write_ctl(xored_status) {
            return Err(Error::illegal_state(format!(
                "Writing to forkserver failed: {err:?}"
            )));
        }

        log::info!("All right - new fork server model version {version} is up");

        let status = forkserver.read_st().map_err(|err| {
            Error::illegal_state(format!("Reading from forkserver failed: {err:?}"))
        })?;

        if status & FS_NEW_OPT_MAPSIZE == FS_NEW_OPT_MAPSIZE {
            let fsrv_map_size = forkserver.read_st().map_err(|err| {
                Error::illegal_state(format!("Failed to read map size from forkserver: {err:?}"))
            })?;
            self.set_map_size(fsrv_map_size)?;
        }

        if status & FS_NEW_OPT_SHDMEM_FUZZ != 0 {
            if input_map_allocated {
                log::info!("Using SHARED MEMORY FUZZING feature.");
                self.uses_shmem_testcase = true;
            } else {
                return Err(Error::illegal_state(
                    "Target requested sharedmem fuzzing, but you didn't prepare shmem",
                ));
            }
        }

        if status & FS_NEW_OPT_AUTODTCT != 0 {
            // Here unlike shmem input fuzzing, we are forced to read things
            // hence no self.autotokens.is_some() to check if we proceed
            let autotokens_size = forkserver.read_st().map_err(|err| {
                Error::illegal_state(format!(
                    "Failed to read autotokens size from forkserver: {err:?}",
                ))
            })?;

            let tokens_size_max = 0xffffff;

            if !(2..=tokens_size_max).contains(&autotokens_size) {
                return Err(Error::illegal_state(format!(
                    "Autotokens size is incorrect, expected 2 to {tokens_size_max} (inclusive), but got {autotokens_size}. Make sure your afl-cc verison is up to date."
                )));
            }
            log::info!("Autotokens size {autotokens_size:x}");
            let buf = forkserver
                .read_st_of_len(autotokens_size as usize)
                .map_err(|err| {
                    Error::illegal_state(format!("Failed to load autotokens: {err:?}"))
                })?;
            if let Some(t) = &mut self.autotokens {
                t.parse_autodict(&buf, autotokens_size as usize);
            }
        }

        let aflx = forkserver.read_st().map_err(|err| {
            Error::illegal_state(format!("Reading from forkserver failed: {err:?}"))
        })?;

        if aflx != initial_status {
            return Err(Error::unknown(format!(
                "Error in forkserver communication ({aflx:?}=>{initial_status:?})",
            )));
        }
        Ok(())
    }

    #[expect(clippy::cast_sign_loss)]
    fn set_map_size(&mut self, fsrv_map_size: i32) -> Result<usize> {
        // When 0, we assume that map_size was filled by the user or const
        /* TODO autofill map size from the observer

        if fsrv_map_size > 0 {
            self.map_size = Some(fsrv_map_size as usize);
        }
        */
        let mut actual_map_size = fsrv_map_size;
        if actual_map_size % 64 != 0 {
            actual_map_size = ((actual_map_size + 63) >> 6) << 6;
        }

        // TODO set AFL_MAP_SIZE
        if let Some(max_size) = self.map_size {
            if actual_map_size as usize > max_size {
                return Err(Error::illegal_state(format!(
                    "The target map size is {actual_map_size} but the allocated map size is {max_size}. \
                    Increase the initial size of the forkserver map to at least that size using the forkserver builder's `coverage_map_size`."
                )));
            }
        } else {
            return Err(Error::illegal_state(format!(
                "The target map size is {actual_map_size} but we did not create a coverage map before launching the target! \
                Set an initial forkserver map to at least that size using the forkserver builder's `coverage_map_size`."
            )));
        }

        // we'll use this later when we truncate the observer
        self.map_size = Some(actual_map_size as usize);

        Ok(actual_map_size as usize)
    }

    #[must_use]
    /// If set to true, we will only spin up a forkserver without any coverage collected. This is useful for several
    /// scenario like slave executors of SAND or cmplog executors.
    pub fn fsrv_only(mut self, fsrv_only: bool) -> Self {
        self.is_fsrv_only = fsrv_only;
        self
    }

    /// Use autodict?
    #[must_use]
    pub fn autotokens(mut self, tokens: &'a mut Tokens) -> Self {
        self.autotokens = Some(tokens);
        self
    }

    /// Set the max input size
    #[must_use]
    pub fn max_input_size(mut self, size: usize) -> Self {
        self.max_input_size = size;
        self
    }

    /// Set the min input size
    #[must_use]
    pub fn min_input_size(mut self, size: usize) -> Self {
        self.min_input_size = size;
        self
    }

    /// Call this if you want to run it under persistent mode; default is false
    #[must_use]
    pub fn is_persistent(mut self, is_persistent: bool) -> Self {
        self.is_persistent = is_persistent;
        self
    }

    /// Treats an execution as a crash if the provided exitcode is returned
    #[must_use]
    pub fn crash_exitcode(mut self, exitcode: i8) -> Self {
        self.crash_exitcode = Some(exitcode);
        self
    }

    /// Call this if the harness uses deferred forkserver mode; default is false
    #[must_use]
    pub fn is_deferred_frksrv(mut self, is_deferred_frksrv: bool) -> Self {
        self.is_deferred_frksrv = is_deferred_frksrv;
        self
    }

    /// Call this to set a default const coverage map size
    #[must_use]
    pub fn coverage_map_size(mut self, size: usize) -> Self {
        self.map_size = Some(size);
        self
    }

    /// Call this to set a signal to be used to kill child processes after executions
    #[must_use]
    pub fn kill_signal(mut self, kill_signal: Signal) -> Self {
        self.kill_signal = Some(kill_signal);
        self
    }

    #[must_use]
    /// Raise the flag to indicate that we use shmem for passing the input over
    pub fn try_use_input_shmem(mut self) -> Self {
        self.try_use_input_shmem = true;
        self
    }
}

impl Default for ForkserverExecutorBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ForkserverExecutorBuilder<'a> {
    /// Creates a new `AFL`-style [`ForkserverExecutor`] with the given target, arguments and observers.
    /// This is the builder for `ForkserverExecutor`
    /// This Forkserver will attempt to provide inputs over shared mem when `shmem_provider` is given.
    /// Else this forkserver will pass the input to the target via `stdin`
    /// in case no input file is specified.
    /// If `debug_child` is set, the child will print to `stdout`/`stderr`.
    #[must_use]
    pub fn new() -> ForkserverExecutorBuilder<'a> {
        ForkserverExecutorBuilder {
            target_inner: StdTargetArgsInner::default(),
            child_env_inner: StdChildArgsInner::default(),
            uses_shmem_testcase: false,
            is_persistent: false,
            is_deferred_frksrv: false,
            is_fsrv_only: false,
            autotokens: None,
            try_use_input_shmem: false,
            map_size: None,
            max_input_size: MAX_INPUT_SIZE_DEFAULT,
            min_input_size: MIN_INPUT_SIZE_DEFAULT,
            kill_signal: None,
            crash_exitcode: None,
        }
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
            StdChildArgs,
            forkserver::{FAILED_TO_START_FORKSERVER_MSG, ForkserverExecutor},
        },
        observers::{ConstMapObserver, HitcountsMapObserver},
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

        let executor = ForkserverExecutor::builder()
            .program(bin)
            .args(args)
            .coverage_map_size(MAP_SIZE)
            .debug_child(false)
            .build::<_>(tuple_list!(edges_observer));

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
