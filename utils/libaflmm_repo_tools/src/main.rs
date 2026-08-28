#![expect(clippy::std_instead_of_core)] // ErrorKind is still unstable in core::io
#![cfg_attr(test, allow(unfulfilled_lint_expectations))]

/*!
 * # `LibAFLmm` repo tools
 *
 * Taking care of the `LibAFLmm` repository since 2026
 */

use clap::Parser;
use colored::Colorize;
use core::str::from_utf8;
use regex::{Regex, RegexSet};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    io,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::{process::Command, task::JoinSet};
use walkdir::{DirEntry, WalkDir};
use which::which;

const REF_LLVM_VERSION: u32 = 20;

#[derive(Parser)]
#[expect(clippy::struct_excessive_bools)]
struct Cli {
    #[arg(short, long)]
    check: bool,
    #[arg(short, long)]
    generate_lockfiles: bool,
    #[arg(long, conflicts_with_all = ["check", "generate_lockfiles", "verbose"])]
    fuzzer_matrix: bool,
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Default)]
struct FuzzerLists {
    standard: Vec<String>,
    qemu: Vec<String>,
}

impl FuzzerLists {
    fn iter(&self) -> impl Iterator<Item = (&'static str, &[String])> {
        [
            ("standard", self.standard.as_slice()),
            ("qemu", self.qemu.as_slice()),
        ]
        .into_iter()
    }

    fn manifests(&self, project_root: &Path) -> HashSet<PathBuf> {
        self.iter()
            .flat_map(|(_, fuzzers)| fuzzers)
            .map(|fuzzer| fuzzer_manifest(project_root, fuzzer))
            .collect()
    }
}

#[derive(Deserialize)]
struct DependabotConfig {
    updates: Vec<DependabotUpdate>,
}

#[derive(Deserialize)]
struct DependabotUpdate {
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default)]
    groups: HashMap<String, serde_yaml::Value>,
}

fn fuzzer_manifest(project_root: &Path, fuzzer: &str) -> PathBuf {
    project_root.join("fuzzers").join(fuzzer).join("Cargo.toml")
}

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo tools must be located in utils/libaflmm_repo_tools")
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_workspace_toml(path: &Path) -> bool {
    for line in read_to_string(path).unwrap().lines() {
        if line.eq("[workspace]") {
            return true;
        }
    }

    false
}

fn is_binary_crate(crate_path: &Path) -> Result<bool, io::Error> {
    if !crate_path.is_dir() {
        return Err(io::Error::new(
            ErrorKind::NotADirectory,
            "Should be a directory.",
        ));
    }

    let main_path = crate_path.to_path_buf().join("src/main.rs");

    Ok(main_path.is_file())
}

fn dependabot_fuzzers(project_root: &Path) -> io::Result<FuzzerLists> {
    let config = read_to_string(project_root.join(".github/dependabot.yml"))?;
    let config: DependabotConfig = serde_yaml::from_str(&config).map_err(io::Error::other)?;
    let mut fuzzers = FuzzerLists::default();

    for update in config.updates {
        let target = if update.groups.contains_key("qemu-fuzzers") {
            &mut fuzzers.qemu
        } else if update.groups.contains_key("standard-fuzzers") {
            &mut fuzzers.standard
        } else {
            continue;
        };

        for directory in update.directories {
            let Some(fuzzer) = directory.strip_prefix("/fuzzers/") else {
                continue;
            };
            let manifest = fuzzer_manifest(project_root, fuzzer);
            if !manifest.is_file() {
                return Err(io::Error::new(
                    ErrorKind::NotFound,
                    format!("manifest does not exist: {}", manifest.display()),
                ));
            }
            target.push(fuzzer.to_string());
        }
    }

    if fuzzers.iter().any(|(_, fuzzers)| fuzzers.is_empty()) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "Dependabot must define standard and qemu fuzzers",
        ));
    }

    Ok(fuzzers)
}

fn should_generate_lockfile(
    cargo_file_path: &Path,
    fuzzers_dir: &Path,
    dependabot_fuzzer_manifests: &HashSet<PathBuf>,
) -> io::Result<bool> {
    if cargo_file_path.starts_with(fuzzers_dir) {
        return Ok(dependabot_fuzzer_manifests.contains(cargo_file_path));
    }

    let cargo_file_dir = cargo_file_path
        .parent()
        .expect("Cargo.toml should always have a parent directory");
    is_binary_crate(cargo_file_dir)
}

async fn run_cargo_generate_lockfile(
    cargo_file_path: PathBuf,
    should_generate: bool,
    verbose: bool,
) -> io::Result<()> {
    // Make sure we parse the correct file
    assert_eq!(
        cargo_file_path.file_name().unwrap().to_str().unwrap(),
        "Cargo.toml"
    );

    if !should_generate {
        if verbose {
            println!(
                "[*] \tSkipping Lockfile for {}...",
                cargo_file_path.as_path().display()
            );
        }
        return Ok(());
    }

    let mut gen_lockfile_cmd = Command::new("cargo");

    gen_lockfile_cmd
        .arg("generate-lockfile")
        .arg("--manifest-path")
        .arg(cargo_file_path.as_path());

    if verbose {
        println!(
            "[*] Generating Lockfile for {}...",
            cargo_file_path.as_path().display()
        );
    }

    let res = gen_lockfile_cmd.output().await?;

    if !res.status.success() {
        let stdout = from_utf8(&res.stdout).unwrap();
        let stderr = from_utf8(&res.stderr).unwrap();
        return Err(io::Error::other(format!(
            "Cargo generate-lockfile failed. Run cargo generate-lockfile for {}.\nstdout: {stdout}\nstderr: {stderr}\ncommand: {gen_lockfile_cmd:?}",
            cargo_file_path.display()
        )));
    }

    Ok(())
}

async fn run_cargo_fmt(cargo_file_path: PathBuf, is_check: bool, verbose: bool) -> io::Result<()> {
    // Make sure we parse the correct file
    assert_eq!(
        cargo_file_path.file_name().unwrap().to_str().unwrap(),
        "Cargo.toml"
    );

    if is_workspace_toml(cargo_file_path.as_path()) {
        println!("[*] Skipping {}...", cargo_file_path.as_path().display());
        return Ok(());
    }

    let task_str = if is_check { "Checking" } else { "Formatting" };

    let mut fmt_command = Command::new("cargo");

    // Use nightly rustfmt so unstable rustfmt.toml features (e.g.
    // `imports_granularity = "Crate"`) actually take effect.
    fmt_command
        .arg("fmt")
        .arg("--manifest-path")
        .arg(cargo_file_path.as_path());

    if is_check {
        fmt_command.arg("--check");
    }

    if verbose {
        println!(
            "[*] {} {}...",
            task_str,
            cargo_file_path.as_path().display()
        );
    }

    let res = fmt_command.output().await?;

    if !res.status.success() {
        let stdout = from_utf8(&res.stdout).unwrap();
        let stderr = from_utf8(&res.stderr).unwrap();
        return Err(io::Error::other(format!(
            "Cargo fmt failed. Run cargo fmt for \"{}\".\nstdout: {stdout}\nstderr: {stderr}\ncommand: {fmt_command:?}",
            cargo_file_path.display()
        )));
    }

    Ok(())
}

async fn run_clang_fmt(
    c_file_path: PathBuf,
    clang: String,
    is_check: bool,
    verbose: bool,
) -> io::Result<()> {
    let task_str = if is_check { "Checking" } else { "Formatting" };

    let mut fmt_command = Command::new(&clang);

    fmt_command
        .arg("-i")
        .arg("--style")
        .arg("file")
        .arg(c_file_path.as_path());

    if is_check {
        fmt_command.arg("-Werror").arg("--dry-run");
    }

    fmt_command.arg(c_file_path.as_path());

    if verbose {
        println!("[*] {} {}...", task_str, c_file_path.as_path().display());
    }

    let res = fmt_command.output().await?;

    if res.status.success() {
        Ok(())
    } else {
        let stdout = from_utf8(&res.stdout).unwrap();
        let stderr = from_utf8(&res.stderr).unwrap();
        println!("{stderr}");
        Err(io::Error::other(format!(
            "{clang} failed.\nstdout:{stdout}\nstderr:{stderr}"
        )))
    }
}

/// extracts (major, minor, patch) version from `clang-format --version` output.
#[must_use]
pub fn parse_llvm_fmt_version(fmt_str: &str) -> Option<(u32, u32, u32)> {
    let re =
        Regex::new(r"clang-format version (?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)").unwrap();
    let caps = re.captures(fmt_str)?;

    Some((
        caps["major"].parse().unwrap(),
        caps["minor"].parse().unwrap(),
        caps["patch"].parse().unwrap(),
    ))
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let libafl_root_dir = project_root();

    if cli.fuzzer_matrix {
        let fuzzers = dependabot_fuzzers(libafl_root_dir)?;
        for (name, fuzzers) in fuzzers.iter() {
            println!("{name}={}", serde_json::to_string(fuzzers).unwrap());
        }
        return Ok(());
    }

    println!(
        "Using \"{}\" as the project root",
        libafl_root_dir.display()
    );
    let rust_excluded_directories = RegexSet::new([
        r".*target.*",
        r".*utils/noaslr.*",
        r".*docs/listings/baby_fuzzer/listing-.*",
        r".*LibAFL/Cargo.toml.*",
        r".*AFLplusplus.*",
    ])
    .expect("Could not create the regex set from the given regex");

    let c_excluded_directories = RegexSet::new([
        r".*target.*",
        r".*libpng-1\.6.*",
        r".*stb_image\.h$",
        r".*dlmalloc\.c$",
        r".*QEMU-Nyx.*",
        r".*AFLplusplus.*",
        r".*Little-CMS.*",
        r".*cms_transform_fuzzer.cc.*",
        r".*sqlite3.*",
        r".*libfuzzer_libmozjpeg.*",
    ])
    .expect("Could not create the regex set from the given regex");

    let c_file_to_format = RegexSet::new([
        r".*\.cpp$",
        r".*\.hpp$",
        r".*\.cc$",
        r".*\.cxx$",
        r".*\.c$",
        r".*\.h$",
    ])
    .expect("Could not create the regex set from the given regex");

    let rust_projects_to_handle: Vec<PathBuf> = WalkDir::new(libafl_root_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !rust_excluded_directories.is_match(&path_str(e.path())))
        .filter(|e| e.file_name() == "Cargo.toml")
        .map(DirEntry::into_path)
        .collect();

    // cargo version
    println!("Using {}", get_version_string("cargo", &[]).await?);

    // rustfmt version
    println!("Using {}", get_version_string("cargo", &["fmt"]).await?);

    let mut tokio_joinset = JoinSet::new();
    let mut all_errors: Vec<String> = Vec::new();

    if cli.generate_lockfiles {
        let fuzzers = dependabot_fuzzers(libafl_root_dir)?;
        let dependabot_fuzzer_manifests = fuzzers.manifests(libafl_root_dir);
        let fuzzers_dir = libafl_root_dir.join("fuzzers");

        for project in rust_projects_to_handle {
            let should_generate =
                should_generate_lockfile(&project, &fuzzers_dir, &dependabot_fuzzer_manifests)?;
            tokio_joinset.spawn(run_cargo_generate_lockfile(
                project,
                should_generate,
                cli.verbose,
            ));
        }
    } else {
        // fallback is for formatting or checking

        let reference_clang_format = format!(
            "clang-format-{}",
            std::env::var("MAIN_LLVM_VERSION")
                .inspect(|e| {
                    println!(
                        "Overriding clang-format version from the default {REF_LLVM_VERSION} to {e} using env variable MAIN_LLVM_VERSION"
                    );
                })
                .unwrap_or(REF_LLVM_VERSION.to_string())
        );
        let unspecified_clang_format = "clang-format";

        let (clang, version, warning) = if which(&reference_clang_format).is_ok() {
            (
                Some(reference_clang_format.as_str()),
                Some(get_version_string(&reference_clang_format, &[]).await?),
                None,
            )
        } else if which(unspecified_clang_format).is_ok() {
            let version_str = get_version_string(unspecified_clang_format, &[]).await?;
            println!("{version_str}");
            let (major, _, _) = parse_llvm_fmt_version(&version_str).unwrap();

            if major == REF_LLVM_VERSION {
                (
                    Some(unspecified_clang_format),
                    Some(version_str.clone()),
                    None,
                )
            } else {
                (
                    Some(unspecified_clang_format),
                    Some(version_str.clone()),
                    Some(format!(
                        "using {version_str}, could provide a different result from {reference_clang_format}"
                    )),
                )
            }
        } else {
            (
                None,
                None,
                Some("clang-format not found. Skipping C formatting...".to_string()),
            )
        };

        if let Some(version) = &version {
            println!("Using {version}");
        }

        let _ = warning.map(print_warning);

        for project in rust_projects_to_handle.clone() {
            tokio_joinset.spawn(run_cargo_fmt(project, cli.check, cli.verbose));
        }

        if let Some(clang) = clang {
            let c_files_to_fmt: Vec<PathBuf> = WalkDir::new(libafl_root_dir)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| !c_excluded_directories.is_match(&path_str(e.path())))
                .filter(|e| e.file_type().is_file())
                .filter(|e| c_file_to_format.is_match(e.file_name().to_str().unwrap()))
                .map(DirEntry::into_path)
                .collect();

            for c_file in c_files_to_fmt {
                tokio_joinset.spawn(run_clang_fmt(
                    c_file,
                    clang.to_string(),
                    cli.check,
                    cli.verbose,
                ));
            }
        }
    }

    all_errors.extend(drain_joinset(&mut tokio_joinset).await);

    if !all_errors.is_empty() {
        report_errors(&all_errors);
    }

    if cli.generate_lockfiles {
        println!("[*] Lockfile generation finished successfully.");
    } else if cli.check {
        println!("[*] Check finished successfully.");
    } else {
        println!("[*] Formatting finished successfully.");
    }

    Ok(())
}

async fn drain_joinset(set: &mut JoinSet<io::Result<()>>) -> Vec<String> {
    let mut errors = Vec::new();
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(err)) => errors.push(err.to_string()),
            Err(join_err) => errors.push(format!("task failed to join: {join_err}")),
        }
    }
    errors
}

/// Print every accumulated error followed by a rustc-style summary line and
/// exit with a non-zero status.
fn report_errors(errors: &[String]) -> ! {
    for msg in errors {
        println!("{msg}\n");
    }
    // Each rendered lint diagnostic starts with `error[...]`; format-task
    // errors are free-form and count as a single error each.
    let total: usize = errors
        .iter()
        .map(|m| m.matches("error[").count().max(1))
        .sum();
    let plural = if total == 1 { "" } else { "s" };
    println!(
        "{}{} aborting due to {total} previous error{plural}",
        "error".red().bold(),
        ":".bold(),
    );
    std::process::exit(exitcode::IOERR);
}

async fn get_version_string(path: &str, args: &[&str]) -> Result<String, io::Error> {
    let res = Command::new(path)
        .args(args)
        .arg("--version")
        .output()
        .await?;
    assert!(
        res.status.success(),
        "Failed to run {path} {args:?}: {res:?}"
    );
    Ok(from_utf8(&res.stdout).unwrap().replace('\n', ""))
}

#[expect(clippy::needless_pass_by_value)]
fn print_warning(warning: String) {
    println!("\n{} {}\n", "Warning:".yellow().bold(), warning);
}
