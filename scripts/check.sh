#!/bin/bash

set -eu

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
LIBAFL_DIR=$(realpath "$SCRIPT_DIR/..")

cd "$SCRIPT_DIR/.." || exit 1

CLIPPY_CMD="RUST_BACKTRACE=full cargo clippy --no-deps --tests --examples --benches"
DOC_CMD="cargo doc --no-deps"

check_md_links() {
    echo "[*] Checking MD links..."

    cd "$LIBAFL_DIR" || exit 1

    if ! command -v linkspector > /dev/null; then
        echo "Error: install linkspector to check MD file links."
        exit 1
    fi

    linkspector check -c "$LIBAFL_DIR/.github/.linkspector.yml" || exit 1

    echo "[*] Done :)"
}

check_for_blobs() {
    blobs=()

    KNOWN_GOOD_FILE_EXTENSIONS=("rs" "c" "h" "cc" "sh" "py" "toml" "yml" "json" "md" "gitignore" "png")

    while read -r file; do
    # NOTE: mimetype detection spawns a perl process for each file and is pretty slow.
    # we work around this by skipping files with known-good extensions.
    ext="${file##*.}"
    for skipExt in "${KNOWN_GOOD_FILE_EXTENSIONS[@]}"; do
        if [ "$ext" = "$skipExt" ]; then
        continue 2
        fi
    done
    if mimetype -b "$file" | grep -Eq "application/(x-object|x-executable)"; then
        blobs+=("$file");
    fi
    done < <(git ls-files --exclude-standard --cached --others)

    if [ ${#blobs[@]} -eq 0 ]
    then
        echo "No object or executable files in the root directory"
    else
        echo "Hey! There are some object or executable file in the root directory!"
        echo "${blobs[@]}"
        echo "Aborting."
        exit 1
    fi
}

# Function to run Clippy on a single directory
run_clippy() {
   local dir="$1"
   local features="$2"
   echo "Running Clippy on $dir"
   echo "$CLIPPY_CMD ${features:+"$features"} -- ${RUSTC_FLAGS:-}"
   pushd "$dir" || return 1

   eval "$CLIPPY_CMD ${features:+"$features"} -- ${RUSTC_FLAGS:-}"

   popd || return 1
}

run_doc() {
   local dir="$1"
   local features="$2"
   echo "Building docs for $dir"
   echo "$DOC_CMD ${features:+"$features"}"
   pushd "$dir" || return 1

   eval "$DOC_CMD ${features:+"$features"}"

   popd || return 1
}

# Define projects based on the operating system
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
   ALL_PROJECTS=(
      "crates/libaflmm"
      "crates/libaflmm_bolts"
      "crates/libaflmm_cc"
      "crates/libaflmm_frida"
      "crates/libaflmm_qemu"
      "crates/libaflmm_qemu/libaflmm_qemu_build"
      "crates/libaflmm_qemu/libaflmm_qemu_sys"
      "crates/libaflmm_nyx"
      "crates/libaflmm_intelpt"
   )
fi

# Do not use --all-features for the following projects
NO_ALL_FEATURES=(
   "crates/libaflmm_qemu"
)

if [ "$#" -eq 0 ]; then
   # No arguments provided, run on all projects
   PROJECTS=("${ALL_PROJECTS[@]}")
else
   # Arguments provided, split the input string into an array
   IFS=',' read -ra PROJECTS <<<"$1"
fi

# Loop through each project and run Clippy
for project in "${PROJECTS[@]}"; do
   # Trim leading and trailing whitespace
   project=$(echo "$project" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
   features="--all-features"
   for item in "${NO_ALL_FEATURES[@]}"; do
     if [[ "$item" == "$project" ]]; then
       features="--features=clippy"
     fi
   done
   if [ -d "$project" ]; then
      run_clippy "$project" "$features"
      run_doc "$project" "$features"
   else
      echo "Warning: Directory $project does not exist. Skipping."
   fi
done

# Last run it on all
eval "$CLIPPY_CMD --workspace --exclude args_reorder --exclude generics_reorder --exclude use_after_mod -- ${RUSTC_FLAGS:-}"
# check docs
eval "$DOC_CMD --workspace --exclude args_reorder --exclude generics_reorder --exclude use_after_mod"

echo "Clippy and doc checks completed for all specified projects."
