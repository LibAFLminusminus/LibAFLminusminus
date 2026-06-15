use crate::{Error, Result};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};
use which::which;

#[cfg(not(target_vendor = "apple"))]
/// The maximum supported LLVM version
pub const LLVM_VERSION_MAX: u32 = 33;

#[cfg(not(target_vendor = "apple"))]
/// The minimum supported LLVM versions
pub const LLVM_VERSION_MIN: u32 = 15;

#[derive(Clone, Debug)]
pub struct LlvmConfig {
    path: PathBuf,
}

impl LlvmConfig {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_file() {
            Err(Error::Unknown(format!(
                "llvm-config not found: {}",
                path.display()
            )))
        } else {
            Ok(LlvmConfig {
                path: path.to_path_buf(),
            })
        }
    }

    /// Search for `llvm-config` in the system.
    ///
    /// It checks:
    /// 1. `LLVM_CONFIG` environment variable.
    /// 2. `llvm-config` in `brew` (`MacOS`).
    /// 3. `llvm-config-VERSION` for versions in `LLVM_VERSION_MIN..=LLVM_VERSION_MAX`.
    /// 4. `llvm-config` in PATH.
    ///
    /// If an exact match is not found, it tries to find the newest available version.
    pub fn find() -> Result<Self> {
        let mut llvm_config: Option<String> = None;

        if let Ok(found) = env::var("LLVM_CONFIG") {
            llvm_config = Some(found)
        }

        // First try to find a version that is >= rustc_llvm_ver if we can determine it,
        // but since this is a generic helper, we just search for all versions.
        // We can prioritize versions if needed.
        if llvm_config.is_none() {
            for version in (LLVM_VERSION_MIN..=LLVM_VERSION_MAX).rev() {
                let llvm_config_name: String = format!("llvm-config-{version}");

                if let Ok(found) = which::which(&llvm_config_name) {
                    llvm_config = Some(found.into_os_string().into_string().unwrap())
                }
            }
        }

        if llvm_config.is_none()
            && let Ok(found) = which("llvm-config")
        {
            llvm_config = Some(found.into_os_string().into_string().unwrap())
        }

        if let Some(llvm_config) = llvm_config {
            let llvm_config = PathBuf::from(llvm_config);

            if !llvm_config.is_file() {
                Err(Error::InvalidArguments(
                    "could not find llvm-config".to_string(),
                ))
            } else {
                Ok(Self { path: llvm_config })
            }
        } else {
            Err(Error::InvalidArguments(
                "could not find llvm-config".to_string(),
            ))
        }
    }

    pub fn exec(&self, args: &[impl AsRef<str>]) -> Result<String> {
        match Command::new(&self.path)
            .args(args.iter().map(|arg| arg.as_ref()))
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8(output.stdout)
                        .expect("Unexpected llvm-config output")
                        .trim()
                        .to_string())
                } else {
                    Err(Error::Unknown(format!(
                        "llvm-config failed with error: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )))
                }
            }
            Err(e) => Err(Error::Unknown(format!(
                "Could not execute {}: {e}",
                self.path.display()
            ))),
        }
    }

    /// Find the LLVM version.
    /// Returns (major, minor)
    pub fn version(&self) -> Result<(u32, u32)> {
        let output = self.exec(&["--version"])?;

        let versions = output.split('.').collect::<Vec<&str>>();

        Ok((versions[0].parse().unwrap(), versions[1].parse().unwrap()))
    }

    /// Find a specific LLVM tool (e.g., `llvm-nm`, `llvm-objcopy`).
    pub fn tool(&self, tool: &str) -> Result<PathBuf> {
        let bindir = self.bin_dir()?;
        let path = Path::new(&bindir).join(tool);

        if !path.is_file() {
            Err(Error::Unknown(format!("could not find tool: {tool}")))
        } else {
            Ok(path)
        }
    }

    /// Get the LLVM bindir.
    pub fn bin_dir(&self) -> Result<PathBuf> {
        let res = PathBuf::from(self.exec(&["--bindir"])?);

        if !res.is_dir() {
            Err(Error::Unknown(format!(
                "Could not find bindir: {}",
                res.display()
            )))
        } else {
            Ok(res)
        }
    }

    pub fn include_dir(&self) -> Result<PathBuf> {
        let res = PathBuf::from(self.exec(&["--includedir"])?);

        if !res.is_dir() {
            Err(Error::Unknown(format!(
                "Could not find includedir: {}",
                res.display()
            )))
        } else {
            Ok(res)
        }
    }

    /// Get the LLVM cxxflags.
    pub fn cxxflags(&self) -> Result<Vec<String>> {
        let cxxflags = self.exec(&["--cxxflags"])?;
        Ok(cxxflags.split_whitespace().map(String::from).collect())
    }

    /// Get the LLVM ldflags.
    pub fn ldflags(&self) -> Result<Vec<String>> {
        let mut llvm_config_ld = vec![];
        llvm_config_ld.push("--ldflags");

        let ldflags = self.exec(&llvm_config_ld)?;
        Ok(ldflags.split_whitespace().map(String::from).collect())
    }

    pub fn clang(&self) -> Result<PathBuf> {
        let bindir = self.bin_dir()?;
        let cc = bindir.join("clang");

        if !cc.is_file() {
            return Err(Error::Unknown(format!(
                "Could not find clang: {}",
                cc.display()
            )));
        }

        Ok(cc)
    }

    pub fn clangpp(&self) -> Result<PathBuf> {
        let bindir = self.bin_dir()?;
        let cxx = bindir.join("clang++");

        if !cxx.is_file() {
            return Err(Error::Unknown(format!(
                "Could not find clang: {}",
                cxx.display()
            )));
        }

        Ok(cxx)
    }
}

/// Execute `rustc` with the given arguments.
///
/// # Panics
/// Panics if `rustc` cannot be executed.
#[must_use]
pub fn exec_rustc(args: &[&str]) -> String {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    match Command::new(rustc).args(args).output() {
        Ok(output) => String::from_utf8(output.stdout)
            .expect("Unexpected rustc output")
            .trim()
            .to_string(),
        Err(e) => panic!("Could not execute rustc: {e}"),
    }
}

/// Find the LLVM version used by `rustc`.
#[must_use]
pub fn find_rustc_llvm_version() -> Option<i32> {
    let output = exec_rustc(&["--verbose", "--version"]);
    let ver = output.split(':').next_back().unwrap().trim();
    if let Some(major) = ver.split('.').collect::<Vec<&str>>().first()
        && let Ok(res) = major.parse::<i32>()
    {
        return Some(res);
    }
    None
}

/// Use `xcrun` to get the path to the Xcode SDK tools library path, for linking
///
/// # Panics
/// Panics if `xcrun` fails to execute.
#[must_use]
pub fn find_macos_sdk_libs() -> String {
    let sdk_path_out = Command::new("xcrun")
        .arg("--show-sdk-path")
        .output()
        .expect("Failed to execute xcrun. Make sure you have Xcode installed and executed `sudo xcode-select --install`");
    format!(
        "-L{}/usr/lib",
        String::from_utf8(sdk_path_out.stdout).unwrap().trim()
    )
}

pub fn build_pass(
    bindir_path: &Path,
    out_dir: &Path,
    cxxflags: &Vec<String>,
    ldflags: &Vec<&str>,
    src_dir: &Path,
    src_file: &str,
) {
    let dot_offset = src_file.rfind('.').unwrap();
    let src_stub = &src_file[..dot_offset];

    let command_result = if cfg!(unix) {
        let r = Command::new(bindir_path.join("clang++"))
            .arg("-v")
            .arg(format!("--target={}", env::var("HOST").unwrap()))
            .args(cxxflags)
            .arg(src_dir.join(src_file))
            .args(ldflags)
            .arg("-o")
            .arg(out_dir.join(format!("{src_stub}.{}", crate::dll_extension())))
            .status();

        Some(r)
    } else if cfg!(windows) {
        let r = Command::new(bindir_path.join("clang-cl.exe"))
            .arg("-v")
            .arg(format!("--target={}", env::var("HOST").unwrap()))
            .args(cxxflags)
            .arg(src_dir.join(src_file))
            .arg("/link")
            .args(ldflags)
            .arg(format!(
                "/OUT:{}",
                out_dir
                    .join(format!("{src_stub}.{}", crate::dll_extension()))
                    .display()
            ))
            .status();
        Some(r)
    } else {
        None
    };

    match command_result {
        Some(res) => match res {
            Ok(s) => {
                assert!(
                    s.success(),
                    "Failed to compile required compiler pass src/{src_file} - Exit status: {s}"
                );
            }
            Err(err) => {
                panic!(
                    "Failed to compile required compiler pass src/{src_file} - Exit status: {err}"
                );
            }
        },
        None => {
            println!(
                "cargo:warning=Skipping compiler pass src/{src_file} - Only supported on Windows or *nix."
            );
        }
    }
}
