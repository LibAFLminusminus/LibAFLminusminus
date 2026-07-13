use crate::version::Version;
use clap::{CommandFactory, Parser, error::ErrorKind};
use core::time::Duration;
use libaflmm::{Result, illegal_argument};
use libaflmm_qemu::prelude::*;
use std::{env, ops::Range, path::PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = format!("qemu_launcher-{}",env!("CPU_TARGET")),
    version = Version::default(),
    about,
    long_about = "Binary fuzzer using QEMU binary instrumentation"
)]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Mode,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Mode {
    Fuzz(FuzzOptions),
    Replay(ReplayOptions),
}

#[derive(clap::Args, Debug, Clone)]
pub struct CommonOptions {
    #[arg(short, long, help = "Input directory")]
    pub input: PathBuf,

    #[clap(short, long, help = "Enable output from the fuzzer clients")]
    pub verbose: bool,

    #[arg(long = "include", help="Include coverage address ranges", value_parser = Cli::parse_ranges)]
    pub include: Option<Vec<Range<GuestAddr>>>,

    #[arg(long = "exclude", help="Exclude coverage address ranges", value_parser = Cli::parse_ranges, conflicts_with="include")]
    pub exclude: Option<Vec<Range<GuestAddr>>>,

    #[arg(last = true, help = "Arguments passed to the target")]
    pub args: Vec<String>,

    #[arg(short = 'd', help = "Write a DrCov Trace.")]
    pub drcov: Option<PathBuf>,

    #[cfg(feature = "injections")]
    #[arg(
        short = 'j',
        long,
        help = "Injections TOML or YAML file definition. Filename must end in .toml or .yaml/.yml."
    )]
    pub injections: Option<PathBuf>,

    #[arg(long, help = "Log file")]
    pub log: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct FuzzOptions {
    #[command(flatten)]
    pub common: CommonOptions,

    #[arg(long, help = "Timeout in milliseconds", default_value = "1000", value_parser = Cli::parse_timeout)]
    pub timeout: Duration,

    #[arg(short = 'x', long, help = "Tokens file")]
    pub tokens: Option<String>,

    #[arg(long, help = "Cpu cores to use", default_value = "all", value_parser = Cores::from_cmdline)]
    pub cores: Cores,

    #[arg(long, help = "Cpu cores to use for ASan", value_parser = Cores::from_cmdline)]
    pub asan_host_cores: Option<Cores>,

    #[arg(long, help = "Cpu cores to use for ASan", value_parser = Cores::from_cmdline)]
    pub asan_guest_cores: Option<Cores>,

    #[arg(long, help = "Cpu cores to use for CmpLog", value_parser = Cores::from_cmdline)]
    pub cmplog_cores: Option<Cores>,

    #[clap(long, help = "Enable use of snapshots to restore state")]
    pub snapshots: bool,

    #[arg(long = "iterations", help = "Maximum number of iterations")]
    pub iterations: Option<u64>,

    #[arg(long = "include-asan", help="Include asan address ranges", value_parser = Cli::parse_ranges)]
    pub include_asan: Option<Vec<Range<GuestAddr>>>,

    #[arg(long = "exclude-asan", help="Exclude asan address ranges", value_parser = Cli::parse_ranges, conflicts_with="include_asan")]
    pub exclude_asan: Option<Vec<Range<GuestAddr>>>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ReplayOptions {
    #[command(flatten)]
    pub common: CommonOptions,
}

impl FuzzOptions {
    pub fn is_asan_host_core(&self, core_id: CoreId) -> bool {
        self.asan_host_cores
            .as_ref()
            .is_some_and(|c| c.contains(core_id))
    }

    pub fn is_asan_guest_core(&self, core_id: CoreId) -> bool {
        self.asan_guest_cores
            .as_ref()
            .is_some_and(|c| c.contains(core_id))
    }

    pub fn is_cmplog_core(&self, core_id: CoreId) -> bool {
        self.cmplog_cores
            .as_ref()
            .is_some_and(|c| c.contains(core_id))
    }

    pub fn validate(&self) {
        if let Some(asan_host_cores) = &self.asan_host_cores {
            for id in asan_host_cores.iter() {
                if !self
                    .cores
                    .contains(id.expect("Only pinned cores are supported"))
                {
                    let mut cmd = Cli::command();
                    cmd.error(
                        ErrorKind::ValueValidation,
                        format!(
                            "Cmplog cores ({:?}) must be a subset of total cores ({:?})",
                            asan_host_cores, self.cores
                        ),
                    )
                    .exit();
                }
            }
        }

        if let Some(cmplog_cores) = &self.cmplog_cores {
            for id in cmplog_cores.iter() {
                if !self
                    .cores
                    .contains(id.expect("Only pinned cores are supported"))
                {
                    let mut cmd = Cli::command();
                    cmd.error(
                        ErrorKind::ValueValidation,
                        format!(
                            "Cmplog cores ({:?}) must be a subset of total cores ({:?})",
                            cmplog_cores, self.cores
                        ),
                    )
                    .exit();
                }
            }
        }
    }
}

impl Cli {
    fn parse_timeout(src: &str) -> Result<Duration> {
        Ok(Duration::from_millis(src.parse()?))
    }

    fn parse_ranges(src: &str) -> Result<Range<GuestAddr>> {
        let parts = src.split('-').collect::<Vec<&str>>();
        if parts.len() == 2 {
            let start = GuestAddr::from_str_radix(parts[0].trim_start_matches("0x"), 16)
                .map_err(|e| illegal_argument!("Invalid start address: {} ({e:})", parts[0]))?;
            let end = GuestAddr::from_str_radix(parts[1].trim_start_matches("0x"), 16)
                .map_err(|e| illegal_argument!("Invalid end address: {} ({e:})", parts[1]))?;
            Ok(Range { start, end })
        } else {
            Err(illegal_argument!("Invalid range provided: {src:}"))
        }
    }

    pub fn validate(&self) {
        if let Mode::Fuzz(fuzz_options) = &self.mode {
            fuzz_options.validate();
        }
    }

    pub fn common(&self) -> &CommonOptions {
        match &self.mode {
            Mode::Fuzz(f) => &f.common,
            Mode::Replay(r) => &r.common,
        }
    }
}
