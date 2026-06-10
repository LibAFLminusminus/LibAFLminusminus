#!/usr/bin/env python3

# simple script to run vale while building the book in mdbook.

import configparser
import json
import os
import shutil
import subprocess
import sys


def env_flag(name):
    return os.environ.get(name, "").strip().lower() in ("1", "true", "yes")


# check if vale should be sync'd.
def needs_sync(vale_ini=".vale.ini"):
    parser = configparser.ConfigParser()
    try:
        with open(vale_ini) as f:
            parser.read_string("[default]\n" + f.read())
    except FileNotFoundError:
        return False
    cfg = parser["default"]
    styles_path = cfg.get("stylespath", ".vale/styles").strip()
    packages = [p.strip() for p in cfg.get("packages", "").split(",") if p.strip()]
    return any(not os.path.isdir(os.path.join(styles_path, p)) for p in packages)


if os.name == "nt":
    os.system("")  # enable ANSI escape processing on windows

if len(sys.argv) > 1 and sys.argv[1] == "supports":
    sys.exit(0)

ci = env_flag("RUN_ON_CI")

ctx, book = json.load(sys.stdin)

if ctx.get("renderer") == "html":
    if shutil.which("vale"):
        if needs_sync():
            print("\033[33mvale: syncing packages...\033[0m", file=sys.stderr)
            subprocess.run(["vale", "sync"], stdout=sys.stderr, stderr=sys.stderr)
        result = subprocess.run(["vale", "src/"], stdout=sys.stderr, stderr=sys.stderr)
        if ci and result.returncode != 0:
            sys.exit(result.returncode)
    elif ci:
        sys.exit("error: vale not found in PATH, required for CI prose checks.")
    else:
        print(
            "\033[33mwarning: vale not found in PATH, install it to see prose lints.\033[0m",
            file=sys.stderr,
        )

json.dump(book, sys.stdout)
