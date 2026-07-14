use std::env;

use crate::{
    Result,
    controllers::{Descriptor, Worker},
    executors::{
        ForkserverChannel, ForkserverExecutor, StdChildArgs, StdChildArgsInner,
        forkserver::{ForkserverConfig, ForkserverShm, KILL_SIGNAL_DEFAULT, report_error_and_exit},
    },
    mutators::Tokens,
    observers::{MapObserver, ObserversTuple},
    runtimes::RuntimeHandle,
};
use libaflmm_bolts::{
    InputLocation, StdTargetArgs, StdTargetArgsInner, SysVShm,
    fs::{INPUTFILE_STD, InputFile},
    tuples::MatchNameRef,
};
use libaflmm_core::{
    Truncate,
    forkserver::{
        FS_NEW_ERROR, FS_NEW_OPT_AUTODTCT, FS_NEW_OPT_MAPSIZE, FS_NEW_OPT_SHDMEM_FUZZ,
        FS_NEW_VERSION_MAGIC_BASE, FS_NEW_VERSION_MAX, FS_NEW_VERSION_MIN, MAX_INPUT_SIZE_DEFAULT,
        MIN_INPUT_SIZE_DEFAULT, SHM_FUZZ_ENV_VAR, SHM_FUZZ_MAP_SIZE_ENV_VAR, SHMEM_FUZZ_HDR_SIZE,
    },
    illegal_argument, illegal_state, runtime,
};
use nix::sys::signal::Signal;

/// Forkserver message. We'll reuse it in a testcase.
pub(crate) const FAILED_TO_START_FORKSERVER_MSG: &str = "Failed to start forkserver";

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

impl<'a> ForkserverExecutorBuilder<'a> {
    /// Builds [`ForkserverExecutor`] downsizing the coverage map to fit exactly the AFL++ map size.
    #[expect(clippy::pedantic)]
    pub fn build_dynamic_map<A, MO, OT, S, W>(
        mut self,
        mut map_observer: A,
        other_observers: OT,
        rt_handle: &RuntimeHandle<S, W>,
    ) -> Result<ForkserverExecutor<(A, OT)>>
    where
        A: AsMut<MO>,
        MO: MapObserver + Truncate,
        OT: ObserversTuple<S>,
        (A, OT): ObserversTuple<S>,
        W: Worker,
    {
        if let Some(dynamic_map_size) = self.map_size {
            map_observer.as_mut().truncate(dynamic_map_size);
        }

        let forkserver = self.build((map_observer, other_observers), rt_handle)?;

        log::info!(
            "ForkserverExecutor: program: {:?}, arguments: {:?}, use_stdin: {:?}, map_size: {:?}",
            forkserver.target,
            forkserver.args,
            self.use_stdin(),
            self.map_size
        );

        if self.uses_shmem_testcase && forkserver.map.is_none() {
            return Err(illegal_state!(
                "Map must always be set for `uses_shmem_testcase`"
            ));
        }

        Ok(forkserver)
    }

    /// Builds [`ForkserverExecutor`].
    /// This Forkserver will attempt to provide inputs over shared mem when [`Self::try_use_input_shmem`] is used.
    /// Else this forkserver will pass the input to the target via `stdin`
    /// in case no input file is specified.
    /// If `debug_child` is set, the child will print to `stdout`/`stderr`.
    #[expect(clippy::pedantic)]
    pub fn build<OT, S, W>(
        &mut self,
        observers: OT,
        rt_handle: &RuntimeHandle<S, W>,
    ) -> Result<ForkserverExecutor<OT>>
    where
        OT: MatchNameRef,
        W: Worker,
    {
        let input_file = match &self.target_inner.input_location {
            InputLocation::StdIn {
                input_file: out_file,
            } => match out_file {
                Some(out_file) => out_file.clone(),
                None => {
                    let root = rt_handle.worker().descriptor().workdir().root_dir();
                    InputFile::create(root.join(INPUTFILE_STD))?
                }
            },
            InputLocation::Arg { argnum: _ } => {
                return Err(illegal_argument!(
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
            illegal_argument!("ForkserverExecutorBuilder::build: target file not found")
        })?;

        let mut forkserver = ForkserverChannel::new(ForkserverConfig {
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
                observers
                    .get(t)
                    .as_ref()
                    .expect("stdout observer not passed in the builder")
                    .as_raw_fd()
                    .expect("only memory fd backend is allowed for forkserver executor")
            }),
            stderr_memfd: self.child_env_inner.stderr_observer.as_ref().map(|t| {
                observers
                    .get(t)
                    .as_ref()
                    .expect("stderr observer not passed in the builder")
                    .as_raw_fd()
                    .expect("only memory fd backend is allowed for forkserver executor")
            }),
            cwd: self.child_env_inner.current_directory.clone(),
            core: self.child_env_inner.core,
        })?;

        // Initial handshake, read 4-bytes hello message from the forkserver.
        let version_status = forkserver
            .read_st()
            .map_err(|err| illegal_state!("{FAILED_TO_START_FORKSERVER_MSG}: {err:?}"))?;

        if (version_status & FS_NEW_ERROR) == FS_NEW_ERROR {
            report_error_and_exit(version_status & 0x0000ffff)?;
        }

        self.initialize_forkserver(version_status, map.is_some(), &mut forkserver)?;

        if self.uses_shmem_testcase && map.is_none() {
            return Err(illegal_state!(
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

        Ok(ForkserverExecutor {
            observers,
            forkserver,
            input_file,
            map,
            target,
            args: self.target_inner.arguments.clone(),
            uses_shmem_testcase: self.uses_shmem_testcase,
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
        forkserver: &mut ForkserverChannel,
    ) -> Result<()> {
        let initial_status = status;
        let version: u32 = status as u32 - FS_NEW_VERSION_MAGIC_BASE;
        match version {
            0 => {
                return Err(illegal_state!(
                    "Fork server version is not assigned, this should not happen. Recompile target.",
                ));
            }
            FS_NEW_VERSION_MIN..=FS_NEW_VERSION_MAX => {
                // good, do nothing
            }
            _ => {
                return Err(illegal_state!(
                    "Fork server version is not supported. Recompile the target.",
                ));
            }
        }

        let xored_status = (status as u32 ^ 0xffffffff) as i32;

        if let Err(err) = forkserver.write_ctl(xored_status) {
            return Err(illegal_state!("Writing to forkserver failed: {err:?}"));
        }

        log::info!("All right - new fork server model version {version} is up");

        let status = forkserver
            .read_st()
            .map_err(|err| illegal_state!("Reading from forkserver failed: {err:?}"))?;

        if status & FS_NEW_OPT_MAPSIZE == FS_NEW_OPT_MAPSIZE {
            let fsrv_map_size = forkserver.read_st().map_err(|err| {
                illegal_state!("Failed to read map size from forkserver: {err:?}")
            })?;
            self.set_map_size(fsrv_map_size)?;
        }

        if status & FS_NEW_OPT_SHDMEM_FUZZ != 0 {
            if input_map_allocated {
                log::info!("Using SHARED MEMORY FUZZING feature.");
                self.uses_shmem_testcase = true;
            } else {
                return Err(illegal_state!(
                    "Target requested sharedmem fuzzing, but you didn't prepare shmem",
                ));
            }
        }

        if status & FS_NEW_OPT_AUTODTCT != 0 {
            // Here unlike shmem input fuzzing, we are forced to read things
            // hence no self.autotokens.is_some() to check if we proceed
            let autotokens_size = forkserver.read_st().map_err(|err| {
                illegal_state!("Failed to read autotokens size from forkserver: {err:?}")
            })?;

            let tokens_size_max = 0xffffff;

            if !(2..=tokens_size_max).contains(&autotokens_size) {
                return Err(illegal_state!(
                    "Autotokens size is incorrect, expected 2 to {tokens_size_max} (inclusive), but got {autotokens_size}. Make sure your afl-cc verison is up to date.",
                ));
            }
            log::info!("Autotokens size {autotokens_size:x}");
            let buf = forkserver
                .read_st_of_len(autotokens_size as usize)
                .map_err(|err| illegal_state!("Failed to load autotokens: {err:?}"))?;
            if let Some(t) = &mut self.autotokens {
                t.parse_autodict(&buf, autotokens_size as usize);
            }
        }

        let aflx = forkserver
            .read_st()
            .map_err(|err| illegal_state!("Reading from forkserver failed: {err:?}"))?;

        if aflx != initial_status {
            return Err(runtime!(
                "Error in forkserver communication ({aflx:?}=>{initial_status:?})",
            ));
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
                return Err(illegal_state!(
                    "The target map size is {actual_map_size} but the allocated map size is {max_size}. \
                    Increase the initial size of the forkserver map to at least that size using the forkserver builder's `coverage_map_size`."
                ));
            }
        } else {
            return Err(illegal_state!(
                "The target map size is {actual_map_size} but we did not create a coverage map before launching the target! \
                Set an initial forkserver map to at least that size using the forkserver builder's `coverage_map_size`."
            ));
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
