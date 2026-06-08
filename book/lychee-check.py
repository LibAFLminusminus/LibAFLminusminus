#!/usr/bin/env python3

import json
import os
import shutil
import subprocess
import sys


def env_flag(name):
    return os.environ.get(name, "").strip().lower() in ("1", "true", "yes")


if os.name == "nt":
    os.system("")  # enable ANSI escape processing on windows

if len(sys.argv) > 1 and sys.argv[1] == "supports":
    sys.exit(0)

ci = env_flag("RUN_ON_CI")

ctx, book = json.load(sys.stdin)

if ctx.get("renderer") == "html":
    if shutil.which("lychee"):
        cmd = ["lychee", "--no-progress"]
        if not (ci or env_flag("LYCHEE_FULL")):
            cmd.append("--offline")
        cmd.append("src/")
        result = subprocess.run(cmd, stdout=sys.stderr, stderr=sys.stderr)
        if ci and result.returncode != 0:
            sys.exit(result.returncode)
    elif ci:
        sys.exit("error: lychee not found in PATH, required for CI link checks.")
    else:
        print(
            "\033[33mwarning: lychee not found in PATH, install it to see link lints.\033[0m",
            file=sys.stderr,
        )

json.dump(book, sys.stdout)
