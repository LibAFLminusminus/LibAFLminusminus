#!/usr/bin/env python3

import shutil
import subprocess
import sys
from pathlib import Path

LIBAFLMM_DIR = Path(__file__).resolve().parent.parent


def run(cmd):
    return subprocess.run(cmd, cwd=LIBAFLMM_DIR, check=False).returncode


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else None
    repo_tools = LIBAFLMM_DIR / "utils" / "libaflmm_repo_tools" / "Cargo.toml"

    if mode == "check":
        if (
            run(
                [
                    "cargo",
                    "run",
                    "--manifest-path",
                    str(repo_tools),
                    "--release",
                    "--",
                    "-c",
                    "--verbose",
                ]
            )
            != 0
        ):
            sys.exit(1)
    elif mode is None:
        if (
            run(
                [
                    "cargo",
                    "run",
                    "--manifest-path",
                    str(repo_tools),
                    "--release",
                    "--",
                    "--verbose",
                ]
            )
            != 0
        ):
            sys.exit(1)
    else:
        print("Error: invalid command.", file=sys.stderr)
        print("Usage:", file=sys.stderr)
        print(f"    {sys.argv[0]} [check]", file=sys.stderr)
        sys.exit(1)

    black_command = None
    if (
        subprocess.run(
            [sys.executable, "-m", "black", "--version"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
        == 0
    ):
        black_command = [sys.executable, "-m", "black"]
    elif shutil.which("black"):
        black_command = ["black"]

    if black_command:
        print("[*] Formatting python files")
        args = ["--extend-exclude", "^/AFLplusplus/"]
        if mode == "check":
            args += ["--check", "--diff"]
        if run(black_command + args + [str(LIBAFLMM_DIR)]) != 0:
            sys.exit(1)
    else:
        print("Warning: python black not found. Formatting skipped for python.")

    if mode != "check" and shutil.which("taplo"):
        print("[*] Formatting TOML files")
        run(["taplo", "format"])

    if shutil.which("just"):
        print("[*] Formatting Justfiles")
        for justfile in sorted(LIBAFLMM_DIR.rglob("Justfile")):
            if mode == "check":
                if (
                    run(
                        [
                            "just",
                            "--unstable",
                            "--fmt",
                            "--check",
                            "--justfile",
                            str(justfile),
                        ]
                    )
                    != 0
                ):
                    sys.exit(1)
            else:
                if (
                    run(["just", "-q", "--justfile", str(justfile), "_check"]) != 0
                    and run(
                        ["just", "--unstable", "--fmt", "--justfile", str(justfile)]
                    )
                    != 0
                ):
                    sys.exit(1)

    print("[*] Done :)")


if __name__ == "__main__":
    main()
