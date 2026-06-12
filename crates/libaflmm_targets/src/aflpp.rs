//! AFLplusplus provided symbols

use std::path::PathBuf;

pub fn aflpp_libs() -> Vec<PathBuf> {
    let out = PathBuf::from(env!("OUT_DIR"));

    assert!(out.is_dir());

    let compiler_rt = out.join("libafl-compiler-rt.a");
    if !compiler_rt.is_file() {
        panic!("{} is not a file.", compiler_rt.display());
    }

    vec![compiler_rt]
}
