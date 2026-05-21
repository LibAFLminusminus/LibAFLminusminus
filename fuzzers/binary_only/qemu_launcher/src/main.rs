//! A libfuzzer-like fuzzer using qemu for binary-only coverage

use crate::{
    fuzzer::QemuFuzzer,
    options::{Cli, Mode},
};
use clap::Parser;
use libaflmm::prelude::*;
use std::env;

mod fuzzer;
mod harness;
mod options;
// mod replay;
mod version;

#[cfg(all(not(miri), debug_assertions))]
#[global_allocator]
static GLOBAL: scudo::GlobalScudoAllocator = scudo::GlobalScudoAllocator;

#[cfg(all(not(miri), not(debug_assertions)))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    cli.validate();

    let env: Vec<(String, String)> = env::vars()
        .filter(|(k, _v)| k != "LD_LIBRARY_PATH")
        .collect::<Vec<(String, String)>>();

    let program = env::args()
        .next()
        .ok_or_else(|| Error::empty_optional("Failed to read program name"))?;

    let mut args = cli.common().args.clone();
    args.insert(0, program);

    // let log = cli.log.as_ref().and_then(|l| {
    //     OpenOptions::new()
    //         .append(true)
    //         .create(true)
    //         .open(l)
    //         .ok()
    //         .map(RefCell::new)
    // });

    // let wrapped_stdout = {
    //     // We forward all outputs to dev/null, but keep a copy around for the fuzzer output.
    //     //
    //     // # Safety
    //     // stdout and stderr should still be open at this point in time.
    //     let (new_stdout, new_stderr) = unsafe { dup_and_mute_outputs()? };

    //     // If we are debugging, re-enable target stderror.
    //     if std::env::var("LIBAFL_FUZZBENCH_DEBUG").is_ok() {
    //         // # Safety
    //         // Nobody else uses the new stderror here.
    //         unsafe {
    //             dup2(new_stderr, io::stderr().as_raw_fd())?;
    //         }
    //     }

    //     // # Safety
    //     // The new stdout is open at this point, and we will don't use it anywhere else.
    //     unsafe { File::from_raw_fd(new_stdout) }
    // };

    // let stdout_cpy = RefCell::new(wrapped_stdout);

    // /* If we are running in verbose, don't provide a replacement stdout, otherwise, use /dev/null */
    // let stdout = if cli.verbose { None } else { Some("/dev/null") };

    match cli.mode {
        Mode::Fuzz(fuzzer_options) => QemuFuzzer::launch(fuzzer_options, env, args),
        Mode::Replay(_replay_options) => {
            // QemuReplay::launch(&cli.common, &replay_options, &env, &args)
            todo!()
        }
    }
}
