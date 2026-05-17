//! A libfuzzer-like fuzzer using qemu for binary-only coverage

use crate::options::FuzzerOptions;
use clap::Parser;
use libaflmm::corpus::OnDiskCorpus;
use libaflmm::corpus::collection::InMemoryOnDiskCorpus;
use libaflmm::corpus::schedulers::{NopScheduler, QueueScheduler};
use libaflmm::inputs::bytes::BytesContext;
use libaflmm::inputs::{BytesInput, InputContext};
use libaflmm::launchers::StdLauncher;
use libaflmm::states::StdState;
use libaflmm::{Result, Worker};
use libaflmm_bolts::os::{dup_and_mute_outputs, dup2};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::{cell::RefCell, fs::OpenOptions};

// mod client;
// mod harness;
// mod instance;
mod options;
mod profile;
mod version;

#[cfg(all(not(miri), debug_assertions))]
#[global_allocator]
static GLOBAL: scudo::GlobalScudoAllocator = scudo::GlobalScudoAllocator;

#[cfg(all(not(miri), not(debug_assertions)))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub fn main() -> Result<()> {
    env_logger::init();

    let cli = FuzzerOptions::parse();
    cli.validate();

    let log = cli.log.as_ref().and_then(|l| {
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(l)
            .ok()
            .map(RefCell::new)
    });

    let wrapped_stdout = {
        // We forward all outputs to dev/null, but keep a copy around for the fuzzer output.
        //
        // # Safety
        // stdout and stderr should still be open at this point in time.
        let (new_stdout, new_stderr) = unsafe { dup_and_mute_outputs()? };

        // If we are debugging, re-enable target stderror.
        if std::env::var("LIBAFL_FUZZBENCH_DEBUG").is_ok() {
            // # Safety
            // Nobody else uses the new stderror here.
            unsafe {
                dup2(new_stderr, io::stderr().as_raw_fd())?;
            }
        }

        // # Safety
        // The new stdout is open at this point, and we will don't use it anywhere else.
        unsafe { File::from_raw_fd(new_stdout) }
    };

    let stdout_cpy = RefCell::new(wrapped_stdout);

    /* If we are running in verbose, don't provide a replacement stdout, otherwise, use /dev/null */
    let stdout = if cli.verbose { None } else { Some("/dev/null") };

    StdLauncher::builder()?
        .cores(cli.cores.clone())
        .timeout(Some(cli.timeout))
        .state_builder(|worker| {
            let scheduler = QueueScheduler::new();
            let crash_dir = worker.workdir().create_dir("crashes")?;
            let queue_dir = worker.workdir().create_dir("queue")?;

            StdState::new(
                BytesContext::default(),
                // Corpus that will be evolved, we keep it in memory for performance
                InMemoryOnDiskCorpus::<BytesInput, QueueScheduler>::builder()
                    .root_dir(queue_dir.as_path())
                    .build::<BytesInput, QueueScheduler>(scheduler)?,
                // Corpus in which we store solutions (crashes in this example),
                // on disk so the user can get them after stopping the fuzzer
                OnDiskCorpus::<BytesInput, NopScheduler>::builder()
                    .root_dir(crash_dir.as_path())
                    .build::<BytesInput, NopScheduler>(NopScheduler)?,
            )
        })
        // .stdout_file(stdout)
        // .stderr_file(stdout)
        .build_inprocess(|rt_handle, state| todo!())?
        .launch()
}
