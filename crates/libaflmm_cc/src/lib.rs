//! Compiler Wrapper from `LibAFL`
#![doc = include_str!("../README.md")]
/*! */

use core::str;
use std::{
    env, io,
    path::Path,
    process::{Command, Output},
    result,
};

pub mod ar;
pub use ar::ArWrapper;

pub mod clang;
pub use clang::{ClangWrapper, LLVMPasses};

pub mod libtool;
pub use libtool::LibtoolWrapper;

pub mod llvm;
pub use llvm::LlvmConfig;

pub type Result<T> = result::Result<T, Error>;

pub mod prelude {
    pub use crate::{
        ArWrapper, ClangWrapper, CompilerWrapper, Configuration, LLVMPasses, LibtoolWrapper,
        ToolWrapper,
    };
}

pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

// TODO macOS
/// extension for static libraries
#[cfg(windows)]
pub const LIB_EXT: &str = "lib";
/// extension for static libraries
#[cfg(not(windows))]
pub const LIB_EXT: &str = "a";

/// prefix for static libraries
#[cfg(windows)]
pub const LIB_PREFIX: &str = "";
/// prefix for static libraries
#[cfg(not(windows))]
pub const LIB_PREFIX: &str = "lib";

/// Wrap a tool hijacking its arguments
pub trait ToolWrapper {
    /// Set the wrapper arguments parsing a command line set of arguments
    fn parse_args(&mut self, args: &[impl AsRef<str>]) -> Result<&'_ mut Self>;

    /// Add an argument
    fn add_arg(&mut self, arg: impl AsRef<str>) -> &'_ mut Self;

    /// Add arguments
    fn add_args<S>(&mut self, args: &[S]) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        for arg in args {
            self.add_arg(arg);
        }
        self
    }

    /// Set directory in which the tool ultimately runs
    fn set_dir(&mut self, dir: impl AsRef<Path>) -> &'_ mut Self;

    /// Directory in which the tool ultimately runs
    fn dir(&self) -> Option<&Path>;

    /// Add a `Configuration`
    fn set_configuration(&mut self, configuration: Configuration) -> &'_ mut Self;

    /// Command to run the compiler
    fn command(&mut self) -> Result<Vec<String>>;

    /// Command to run the compiler for a given `Configuration`
    fn command_for_configuration(&mut self, configuration: Configuration) -> Result<Vec<String>>;

    /// Get the requested `Configuration`
    fn configuration(&self) -> Result<Configuration>;

    /// Whether to ignore the configured `Configurations`. Useful for e.g. nested calls to
    /// `libaflmm_cc` from `libaflmm_libtool`.
    fn ignore_configurations(&self) -> Result<bool>;

    /// Get if in linking mode
    fn is_linking(&self) -> bool;

    /// Filter out argumets
    fn filter(&self, _args: &mut Vec<String>) {}

    /// Silences `libaflmm_cc` output
    fn silence(&mut self, value: bool) -> &'_ mut Self;

    /// Returns `true` if `silence` was called with `true`
    fn is_silent(&self) -> bool;

    /// Run the tool
    fn run(&mut self) -> Result<Output> {
        let configuration = if self.ignore_configurations()? {
            Configuration::Default
        } else {
            self.configuration()?
        };

        let mut args = self.command_for_configuration(configuration)?;
        self.filter(&mut args);

        if !self.is_silent() {
            dbg!(args.clone());
        }

        if args.is_empty() {
            return Err(Error::InvalidArguments(
                "The number of arguments cannot be 0".into(),
            ));
        }

        let mut cmd = Command::new(&args[0]);

        cmd.args(&args[1..]);

        if let Some(dir) = self.dir() {
            cmd.current_dir(dir);
        }

        let output = cmd.output()?;

        if !self.is_silent() {
            dbg!(output.status);
        }

        Ok(output)
    }
}

/// Wrap a compiler hijacking its arguments
pub trait CompilerWrapper: ToolWrapper {
    /// Add a compiler argument only when compiling
    fn add_cc_arg(&mut self, arg: impl AsRef<str>) -> &'_ mut Self;

    /// Add a compiler argument only when linking
    fn add_link_arg(&mut self, arg: impl AsRef<str>) -> &'_ mut Self;

    /// Add compiler arguments only when compiling
    fn add_cc_args(&mut self, args: &[impl AsRef<str>]) -> &'_ mut Self {
        for arg in args {
            self.add_cc_arg(arg);
        }
        self
    }

    /// Add compiler arguments only when linking
    fn add_link_args(&mut self, args: &[impl AsRef<str>]) -> &'_ mut Self {
        for arg in args {
            self.add_link_arg(arg);
        }
        self
    }

    fn add_include(&mut self, include_dir: impl AsRef<Path>) -> &'_ mut Self;

    fn define(&mut self, define: impl AsRef<str>, value: impl AsRef<str>) -> &'_ mut Self;

    /// Link static C lib
    fn link_staticlib(&mut self, lib: impl AsRef<Path>) -> &'_ mut Self;

    fn link_staticlibs(
        &mut self,
        libs: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> &'_ mut Self {
        for lib in libs.into_iter() {
            self.link_staticlib(lib);
        }

        self
    }

    /// Finds the current `python3` version and adds `-lpython3.<version>` as linker argument.
    /// Useful for fuzzers that need libpython, such as `nautilus`-based fuzzers.
    fn link_libpython(&mut self) -> result::Result<&'_ mut Self, String> {
        Ok(self.add_link_arg(format!("-l{}", find_python3_version()?)))
    }
}

/// `LibAFL` CC Error Type
#[derive(Debug)]
pub enum Error {
    /// CC Wrapper called with invalid arguments
    InvalidArguments(String),
    /// Io error occurred
    Io(std::io::Error),
    /// Something else happened
    Unknown(String),
}

/// `LibAFL` target configuration
#[derive(Debug, Clone)]
pub enum Configuration {
    /// Default uninstrumented configurations
    Default,
    /// Sanitizing addresses
    AddressSanitizer,
    /// Sanitizing undefined behavior
    UndefinedBehaviorSanitizer,
    /// Generating a coverage map
    GenerateCoverageMap,
    /// Generating coverage profile data for `llvm-cov`
    GenerateCoverageProfile,
    /// Instrumenting for cmplog/redqueen
    CmpLog,
    /// A compound `Configuration`, made up of a list of other `Configuration`s
    Compound(Vec<Self>),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl Configuration {
    /// Get compiler flags for this `Configuration`
    pub fn to_flags(&self) -> Result<Vec<String>> {
        Ok(match self {
            Configuration::Default => vec![],
            // hardware asan is more memory efficient than asan on arm64
            #[cfg(all(
                any(target_os = "linux", target_os = "android"),
                target_arch = "aarch64"
            ))]
            Configuration::AddressSanitizer => vec!["-fsanitize=hwaddress".to_string()],
            #[cfg(not(all(
                any(target_os = "linux", target_os = "android"),
                target_arch = "aarch64"
            )))]
            Configuration::AddressSanitizer => vec!["-fsanitize=address".to_string()],
            Configuration::UndefinedBehaviorSanitizer => vec!["-fsanitize=undefined".to_string()],
            Configuration::GenerateCoverageMap => {
                vec!["-fsanitize-coverage=trace-pc-guard".to_string()]
            }
            Configuration::CmpLog => vec!["-fsanitize-coverage=trace-cmp".to_string()],
            Configuration::GenerateCoverageProfile => {
                vec![
                    "-fprofile-instr-generate".to_string(),
                    "-fcoverage-mapping".to_string(),
                ]
            }
            Configuration::Compound(configurations) => {
                let mut result: Vec<String> = vec![];
                for configuration in configurations {
                    result.extend(configuration.to_flags()?);
                }
                result
            }
        })
    }
    /// Insert a `Configuration` specific 'tag' in the extension of the given file
    #[must_use]
    pub fn replace_extension(&self, path: &Path) -> std::path::PathBuf {
        let mut parent = if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            std::path::PathBuf::from("")
        };
        let output = path.file_name().unwrap();
        let output = output.to_str().unwrap();

        let new_filename = if let Some((filename, extension)) = output.split_once('.') {
            if let Configuration::Default = self {
                format!("{filename}.{extension}")
            } else {
                format!("{filename}.{self}.{extension}")
            }
        } else if let Configuration::Default = self {
            output.to_string()
        } else {
            format!("{output}.{self}")
        };
        parent.push(new_filename);
        parent
    }
}

impl str::FromStr for Configuration {
    type Err = ();
    fn from_str(input: &str) -> result::Result<Configuration, Self::Err> {
        Ok(match input {
            "asan" => Configuration::AddressSanitizer,
            "ubsan" => Configuration::UndefinedBehaviorSanitizer,
            "coverage" => Configuration::GenerateCoverageMap,
            "llvm-cov" => Configuration::GenerateCoverageProfile,
            "cmplog" => Configuration::CmpLog,
            _ => Configuration::Default,
        })
    }
}

impl core::fmt::Display for Configuration {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Configuration::Default => write!(f, ""),
            Configuration::AddressSanitizer => write!(f, "asan"),
            Configuration::UndefinedBehaviorSanitizer => write!(f, "ubsan"),
            Configuration::GenerateCoverageMap => write!(f, "coverage"),
            Configuration::GenerateCoverageProfile => write!(f, "llvm-cov"),
            Configuration::CmpLog => write!(f, "cmplog"),
            Configuration::Compound(configurations) => {
                let mut result: Vec<String> = vec![];
                for configuration in configurations {
                    result.push(format!("{configuration}"));
                }
                write!(f, "{}", result.join("_"))
            }
        }
    }
}

/// Helper function to find the current python3 version, if you need this information at link time.
/// Example output: `python3.11`
/// Example use: `.add_link_arg(format!("-l{}", find_python3_version()?))`
/// Hint: you can use `link_libpython()` directly.
fn find_python3_version() -> result::Result<String, String> {
    match Command::new("python3").arg("--version").output() {
        Ok(output) => {
            let python_version = str::from_utf8(&output.stdout).unwrap_or_default().trim();
            if python_version.is_empty() {
                return Err("Empty return from python3 --version".to_string());
            }
            let version = python_version.split("Python 3.").nth(1).ok_or_else(|| {
                format!("Could not find Python 3 in version string: {python_version}")
            })?;
            let mut version = version.split('.');
            let version = version.next().ok_or_else(|| {
                format!("Could not split python3 version string {python_version}")
            })?;
            Ok(format!("python3.{version}"))
        }
        Err(err) => Err(format!("Could not execute python3 --version: {err:?}")),
    }
}

/// Get the extension for a shared object (dll, so, dylib)
///
/// # Panics
/// Panics if the target family is unsupported (not windows or unix).
#[must_use]
pub fn dll_extension<'a>() -> &'a str {
    if let Ok(vendor) = env::var("CARGO_CFG_TARGET_VENDOR")
        && vendor == "apple"
    {
        return "dylib";
    }

    let family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_else(|_| "unknown".into());

    match family.as_str() {
        "windows" => "dll",
        "unix" => "so",
        _ => panic!("Unsupported target family: {family}"),
    }
}
