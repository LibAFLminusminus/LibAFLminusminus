//! AFLplusplus provided symbols

use libaflmm_cc::{LlvmConfig, prelude::*};
use std::{
    io,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
};
use tar::Archive;
use zstd::Decoder;

pub const AFLPP_SRC_HASH: &str = env!("AFLPP_SRC_HASH");
const AFLPP_SRC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/aflpp.tar.zst"));

pub fn aflpp_hash() -> &'static str {
    AFLPP_SRC_HASH
}

/// Returns the pcguard shared library pass' path
pub fn compile_aflpp_pcguard(
    llvm_config: &LlvmConfig,
    cache_dir: impl AsRef<Path>,
) -> libaflmm_cc::Result<PathBuf> {
    let root = cache_dir.as_ref();
    let out = root.join("SanitizerCoveragePCGUARD.so");

    if out.is_file() {
        return Ok(out);
    }

    let obj_files = &[
        "instrumentation/afl-llvm-common.cc",
        "instrumentation/SanitizerCoveragePCGUARD.so.cc",
    ];

    let hdr_files = &[
        "include/config.h",
        "include/types.h",
        "include/debug.h",
        "instrumentation/afl-llvm-common.h",
        "instrumentation/PathCoverage.h",
    ];

    let mut pcguard_files = obj_files.to_vec();
    pcguard_files.extend(hdr_files);

    let mut archive = Archive::new(Decoder::new(AFLPP_SRC)?);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if pcguard_files.iter().any(|f| path == Path::new(f)) {
            entry.unpack_in(root)?;
        }
    }

    let (llvm_major, llvm_minor) = llvm_config.version()?;
    let mut cc = ClangWrapper::try_from(llvm_config)?;

    let res = cc
        .cpp(true)
        .silence(false)
        .set_dir(&cache_dir)
        .include_llvm_headers(true)
        .add_include(cache_dir.as_ref().join("include"))
        .define("LLVM_MAJOR", llvm_major.to_string())
        .define("LLVM_MINOR", llvm_minor.to_string())
        .add_args(&[
            "-fno-rtti",
            "-fno-exceptions",
            "-fPIC",
            "-shared",
            "-o",
            "SanitizerCoveragePCGUARD.so",
            "-Wno-deprecated-declarations",
            "-Wdeprecated",
            "-Wno-deprecated-copy-dtor",
            "-Wno-deprecated-copy-with-dtor",
        ])
        .add_args(obj_files)
        .run()
        .unwrap();

    if !res.status.success() {
        let out = String::from_utf8(res.stdout.clone()).unwrap();
        let err = String::from_utf8(res.stderr.clone()).unwrap();

        eprintln!("stdout: {}", out);
        eprintln!("stderr: {}", err);

        Err(io::Error::from_raw_os_error(res.status.into_raw()).into())
    } else {
        Ok(out)
    }
}
