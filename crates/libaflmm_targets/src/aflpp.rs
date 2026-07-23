//! AFLplusplus provided symbols

use libaflmm_cc::{LlvmConfig, Result, prelude::*};
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

enum OutputFmt {
    Shared,
    Object,
}

fn compile_aflpp_helper(
    llvm_config: &LlvmConfig,
    cache_dir: impl AsRef<Path>,
    outfile: impl AsRef<str>,
    obj_files: &[impl AsRef<str>],
    hdr_files: &[impl AsRef<str>],
    output_fmt: OutputFmt,
    is_cpp: bool,
) -> Result<PathBuf> {
    let root = cache_dir.as_ref();
    let out = root.join(outfile.as_ref());

    if out.is_file() {
        return Ok(out);
    }

    let mut pcguard_files: Vec<&str> = obj_files.iter().map(|obj| obj.as_ref()).collect();
    pcguard_files.extend(hdr_files.iter().map(|h| h.as_ref()));

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

    match output_fmt {
        OutputFmt::Shared => {
            cc.add_args(&["-fPIC", "-shared"]);
        }
        OutputFmt::Object => {
            cc.add_arg("-c");
        }
    }

    if is_cpp {
        cc.add_args(&["-fno-rtti", "-fno-exceptions"]);
    }

    let res = cc
        .cpp(false)
        .silence(true)
        .set_dir(&cache_dir)
        .include_llvm_headers(true)
        .add_include(cache_dir.as_ref().join("include"))
        .define("LLVM_MAJOR", llvm_major.to_string())
        .define("LLVM_MINOR", llvm_minor.to_string())
        .add_args(&[
            "-o",
            outfile.as_ref(),
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

pub fn compile_aflpp_compiler_rt(
    llvm_config: &LlvmConfig,
    cache_dir: impl AsRef<Path>,
) -> Result<PathBuf> {
    let obj_files = &["instrumentation/afl-compiler-rt.o.c"];

    let hdr_files = &[
        "include/config.h",
        "include/types.h",
        "include/debug.h",
        "include/cmplog.h",
        "include/afl-ijon-min.h",
        "include/bug-pass.h",
        "instrumentation/afl-llvm-common.h",
        "instrumentation/llvm-alternative-coverage.h",
        "instrumentation/PathCoverage.h",
    ];

    compile_aflpp_helper(
        llvm_config,
        cache_dir,
        "afl-compiler-rt.o",
        obj_files,
        hdr_files,
        OutputFmt::Object,
        false,
    )
}

/// Returns the pcguard shared library pass' path
pub fn compile_aflpp_pcguard(
    llvm_config: &LlvmConfig,
    cache_dir: impl AsRef<Path>,
) -> Result<PathBuf> {
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

    compile_aflpp_helper(
        llvm_config,
        cache_dir,
        "SanitizerCoveragePCGUARD.so",
        obj_files,
        hdr_files,
        OutputFmt::Shared,
        true,
    )
}
