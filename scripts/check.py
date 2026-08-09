#!/usr/bin/env python3

import os
import subprocess
import sys
from pathlib import Path

LIBAFLMM_DIR = Path(__file__).resolve().parent.parent

CLIPPY_CMD = ["cargo", "clippy", "--no-deps", "--tests", "--examples", "--benches"]
DOC_CMD = ["cargo", "doc", "--no-deps"]

RUSTC_FLAGS = os.environ.get("RUSTC_FLAGS", "").split()
CLIPPY_ENV = dict(os.environ, RUST_BACKTRACE="full")

ALL_PROJECTS = [
    "crates/libaflmm",
    "crates/libaflmm_bolts",
    "crates/libaflmm_cc",
    "crates/libaflmm_frida",
    "crates/libaflmm_qemu",
    "crates/libaflmm_qemu/libaflmm_qemu_build",
    "crates/libaflmm_qemu/libaflmm_qemu_sys",
    "crates/libaflmm_nyx",
    "crates/libaflmm_intelpt",
]

# do not use --all-features
NO_ALL_FEATURES = ["crates/libaflmm_qemu"]

def run(cmd, cwd, env=None):
    print(" ".join(cmd))
    if subprocess.run(cmd, cwd=cwd, env=env, check=False).returncode != 0:
        sys.exit(1)


def run_clippy(directory, features):
    print(f"Running Clippy on {directory}")
    run(CLIPPY_CMD + features + ["--"] + RUSTC_FLAGS, cwd=directory, env=CLIPPY_ENV)


def run_doc(directory, features):
    print(f"Building docs for {directory}")
    run(DOC_CMD + features, cwd=directory)


def main():
    if len(sys.argv) > 1:
        projects = [p.strip() for p in sys.argv[1].split(",")]
    else:
        projects = ALL_PROJECTS

    for project in projects:
        if project in NO_ALL_FEATURES:
            features = ["--features=clippy"]
        else:
            features = ["--all-features"]

        if (LIBAFLMM_DIR / project).is_dir():
            run_clippy(LIBAFLMM_DIR / project, features)
            run_doc(LIBAFLMM_DIR / project, features)
        else:
            print(f"Directory {project} does not exist.")
            sys.exit(1)

    run_clippy(LIBAFLMM_DIR, ["--workspace", "--exclude", "generics_reorder"])
    run_doc(LIBAFLMM_DIR, ["--workspace", "--exclude", "generics_reorder"])

    print("Clippy and doc checks completed for all specified projects.")


if __name__ == "__main__":
    main()
