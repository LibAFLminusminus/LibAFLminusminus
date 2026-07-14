use crate::executors::forkserver::ForkserverConfig;
use crate::{Result, executors::Config};
use libaflmm_core::forkserver::{
    AFL_GCC_ONLY_FSRV_VAR, AFL_LLVM_ONLY_FSRV_VAR, AFL_MAP_SIZE_ENV_VAR,
};
use libaflmm_core::{forkserver::SHM_ENV_VAR, illegal_argument, unknown};
use libaflmm_core::{illegal_state, runtime};
use nix::sys::select::{FdSet, pselect};
use nix::sys::signal::{SigSet, kill};
use nix::sys::time::TimeSpec;
use nix::sys::wait::waitpid;
use nix::{sys::signal::Signal, unistd::Pid};
use std::io::{self, PipeReader, PipeWriter, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::{
    env,
    process::{Child, Command, Stdio},
};

/// The [`ForkserverChannel`] is communication channel with a child process that forks on request of the fuzzer.
/// The communication happens via pipe.
#[derive(Debug)]
pub struct ForkserverChannel {
    /// The "actual" forkserver we spawned in the target
    fsrv_handle: Child,
    /// Status pipe
    st_reader: PipeReader,
    /// Control pipe
    ctl_writer: PipeWriter,
    /// Pid of the current forked child (child of the forkserver) during execution
    child_pid: Option<Pid>,
    /// The last status reported to us by the in-target forkserver
    status: i32,
    /// If the last run timed out (in in-target i32)
    last_run_timed_out: i32,
    /// The signal this [`Forkserver`] will use to kill
    kill_signal: Signal,
}

impl Drop for ForkserverChannel {
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

impl ForkserverChannel {
    /// Create a new [`ForkserverChannel`] that will kill child processes
    /// with the given `kill_signal`.
    /// Using `Forkserver::new(..)` will default to [`Signal::SIGTERM`].
    pub fn new(cfg: ForkserverConfig) -> Result<Self> {
        let ForkserverConfig {
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
            return Err(unknown!(
                "Coverage map size unknown. Use coverage_map_size() to tell the forkserver about the map size.",
            ));
        };

        if env::var(SHM_ENV_VAR).is_err() {
            return Err(unknown!(
                "__AFL_SHM_ID not set. It is necessary to set this env, otherwise the forkserver cannot communicate with the fuzzer",
            ));
        }

        let afl_debug = if let Ok(afl_debug) = env::var("AFL_DEBUG") {
            if afl_debug != "1" && afl_debug != "0" {
                return Err(illegal_argument!("AFL_DEBUG must be either 1 or 0"));
            }
            afl_debug == "1"
        } else {
            false
        };

        let (st_reader, st_writer) = io::pipe()?;
        let (ctl_reader, ctl_writer) = io::pipe()?;

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

        // # Safety
        //
        // The pipe file descriptors used for `setpipe` are valid at this point.
        let fsrv_handle = unsafe {
            match Config::setsid(
                command
                    .env("LD_BIND_NOW", "1")
                    .envs(envs)
                    .setlimit(memlimit)
                    .set_coredump(afl_debug),
            )
            .setpipe(st_writer.as_raw_fd(), ctl_reader.as_raw_fd())
            .spawn()
            {
                Ok(fsrv_handle) => fsrv_handle,
                Err(err) => {
                    return Err(illegal_state!("Could not spawn the forkserver: {err:#?}"));
                }
            }
        };

        Ok(Self {
            fsrv_handle,
            st_reader,
            ctl_writer,
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

    /// The kill signal
    #[must_use]
    pub fn kill_signal(&self) -> Signal {
        self.kill_signal
    }

    /// Read from the st pipe
    pub fn read_st(&mut self) -> Result<i32> {
        let mut buf: [u8; 4] = [0_u8; 4];
        let rlen = self.st_reader.read(&mut buf)?;

        if rlen == size_of::<i32>() {
            Ok(i32::from_ne_bytes(buf))
        } else {
            // NOTE: The underlying API does not guarantee that the read will return
            //       exactly four bytes, but the chance of this happening is very low.
            //       This is a sacrifice of correctness for performance.
            Err(illegal_state!(
                "Could not read from st pipe. Expected {} bytes, got {rlen} bytes",
                size_of::<i32>()
            ))
        }
    }

    /// Read bytes of any length from the st pipe
    pub fn read_st_of_len(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        self.st_reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Write to the ctl pipe
    pub fn write_ctl(&mut self, val: i32) -> Result<()> {
        let slen = self.ctl_writer.write(&val.to_ne_bytes())?;

        if slen == size_of::<i32>() {
            Ok(())
        } else {
            // NOTE: The underlying API does not guarantee that exactly four bytes
            //       are written, but the chance of this happening is very low.
            //       This is a sacrifice of correctness for performance.
            Err(illegal_state!(
                "Could not write to ctl pipe. Expected {} bytes, wrote {slen} bytes",
                size_of::<i32>()
            ))
        }
    }

    /// Read a message from the child process.
    pub fn read_st_timeout(&mut self, timeout: &TimeSpec) -> Result<Option<i32>> {
        let mut buf: [u8; 4] = [0_u8; 4];

        let st_read = self.st_reader.as_fd();

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
            if self.st_reader.read_exact(&mut buf).is_ok() {
                let val: i32 = i32::from_ne_bytes(buf);
                Ok(Some(val))
            } else {
                Err(runtime!("Unable to communicate with fork server (OOM?)"))
            }
        } else {
            Ok(None)
        }
    }
}
