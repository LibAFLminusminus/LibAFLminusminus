#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
cd "$SCRIPT_DIR/.." || exit 1

# TODO: This should be rewritten in rust, a Makefile, or some platform-independent language

echo "Welcome to the happy clean script. :)"

failed=0

while IFS= read -r lockfile; do
    rust_dir=$(dirname "$lockfile")

    if [ ! -f "$rust_dir/Cargo.toml" ]; then
        echo "[!] Skipping $rust_dir, no Cargo.toml next to its Cargo.lock"
        continue
    fi

    echo "[*] Running clean for $rust_dir"
    if ! cargo clean --manifest-path "$rust_dir/Cargo.toml"; then
        echo "[!] Clean failed for $rust_dir"
        failed=1
    fi
done < <(find . -name Cargo.lock -not -path '*/target/*' -not -path '*/AFLplusplus/*')

if [ "$failed" -ne 0 ]; then
    echo "[!] Some crates could not be cleaned"
    exit 1
fi

echo "[*] Done :)"
