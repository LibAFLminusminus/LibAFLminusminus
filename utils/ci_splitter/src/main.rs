use core::error::Error;
use std::{
    env,
    process::{Command, exit},
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        exit(1);
    }
    let instance_idx: usize = args[1]
        .parse()
        .map_err(|e| format!("Failed to parse instance index '{}': {}", args[1], e))?;

    let ci_instances: usize = if let Ok(val) = env::var("CI_INSTANCES") {
        val.parse()
            .map_err(|e| format!("CI_INSTANCES must be a positive integer, got '{val}': {e}"))?
    } else {
        eprintln!("Error: CI_INSTANCES environment variable not set");
        exit(1);
    };

    let llvm_var: usize = if let Ok(val) = env::var("LLVM_VERSION") {
        val.parse()
            .map_err(|e| format!("LLVM_VERSION must be a positive integer, got '{val}': {e}"))?
    } else {
        eprintln!("Error: LLVM_VERSION environment variable not set");
        exit(1);
    };

    if env::var("LLVM_CONFIG").is_err() {
        unsafe {
            env::set_var("LLVM_CONFIG", format!("llvm-config-{llvm_var}"));
        }
    }

    let common_exclude_features = [
        "prelude",
        "python",
        "sancov_pcguard_edges",
        "arm",
        "aarch64",
        "i386",
        "be",
        "systemmode",
        "whole_archive",
    ];
    let exclude_features_str = common_exclude_features.join(",");

    let lqemu_exclude_features = ["slirp", "intel_pt", "intel_pt_export_raw", "nyx"];
    let lqemu_exclude_features: Vec<&str> = lqemu_exclude_features
        .into_iter()
        .chain(common_exclude_features)
        .collect();
    let lqemu_exclude_features_str = lqemu_exclude_features.join(",");

    // libaflmm_asan_libc needs no_std; qemu crates are checked separately below;
    // frida, linters, and pylibaflmm require special build environments
    let the_command = format!(
        "DOCS_RS=1 cargo hack check --workspace --each-feature --clean-per-run \
            --exclude-features={exclude_features_str} \
            --no-dev-deps \
            --exclude libaflmm_qemu --exclude libaflmm_qemu_sys --exclude libaflmm_qemu_build \
            --exclude libaflmm_qemu_runner --exclude libvharness_sys \
            --exclude libaflmm_asan_libc --exclude libaflmm_asan_fuzz \
            --exclude libaflmm_frida \
            --exclude args_reorder --exclude generics_reorder --exclude use_after_mod \
            --exclude pylibaflmm \
            --print-command-list; "
    ) + &format!(
        "DOCS_RS=1 cargo hack check -p libaflmm_qemu -p libaflmm_qemu_sys --each-feature --clean-per-run \
            --exclude-features={lqemu_exclude_features_str} \
            --no-dev-deps --features usermode --print-command-list"
    );

    let output = Command::new("sh").arg("-c").arg(&the_command).output()?;
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        exit(output.status.code().unwrap_or(1));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();

    let all_task_cnt = lines.len() / 2; // one task == two lines
    let task_per_core = all_task_cnt / ci_instances;
    println!("{task_per_core}/{all_task_cnt} tasks assigned to this instance");

    let start = instance_idx * 2 * task_per_core;
    let end = ((instance_idx + 1) * 2 * task_per_core).min(lines.len());
    let total_lines = end - start;
    for (idx, task) in lines[start..end].iter().enumerate() {
        println!("Running task {} / {total_lines}: \"{task}\"", idx + 1);

        // skip no_std crates that have no features (checking them requires panic=abort)
        if task.contains("--no-default-features") && !task.contains("--features") {
            continue;
        }

        // run each task, with DOCS_RS override for libaflmm_frida
        let mut cmd = Command::new("bash");
        cmd.arg("-c");
        if task.contains("libaflmm_frida") {
            cmd.env("DOCS_RS", "1");
            let task = task.replace("cargo ", "cargo +nightly ");
            cmd.arg(task);
        } else {
            cmd.arg(task);
        }
        let status = cmd.status()?;
        if !status.success() {
            return Err(format!("Command failed (exit code {:?}): {}", status.code(), task).into());
        }
    }

    Ok(())
}
