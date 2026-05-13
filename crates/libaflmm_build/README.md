# `libaflmm_build`: Build-time utilities for `LibAFLmm`

The `libaflmm_build` crate provides build-time utilities for `LibAFL--`, specifically for detecting LLVM tools and versions. It is used by other `LibAFL--` crates to ensure they are built with the correct LLVM configuration.

## Features

*   **LLVM Tool Detection**: Finds `llvm-config`, `llvm-nm`, `llvm-objcopy`, and other tools, handling versioned binaries (e.g., `llvm-config-15`) and platform-specific paths (e.g., Homebrew on macOS).
*   **Version Detection**: Detects the system LLVM version and the LLVM version used by `rustc`.

## Examples

```rust
use libafl_build::{find_llvm_config, find_llvm_tool, find_llvm_version};

// Find llvm-config
if let Ok(path) = find_llvm_config() {
    println!("Found llvm-config at: {}", path);
}

// Find a specific tool like llvm-nm
if let Ok(path) = find_llvm_tool("llvm-nm") {
    println!("Found llvm-nm at: {}", path);
}

// Check LLVM version
if let Some(version) = find_llvm_version() {
    println!("Detected LLVM version: {}", version);
}
```

## The `LibAFLmm` Project

This crate is part of the [LibAFLmm project](https://github.com/LibAFLminusminus/LibAFLminusminus).

The [README](../../README.md) contains the list of maintainers and licensing information.
