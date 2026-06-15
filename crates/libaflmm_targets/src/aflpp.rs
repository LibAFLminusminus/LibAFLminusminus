//! AFLplusplus provided symbols

use libaflmm_cc::prelude::*;
use std::{io, path::Path};
use tar::Archive;
use zstd::Decoder;

const AFLPP_SRC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/aflpp.tar.zst"));
const AFLPP_SRC_HASH: &str = env!("AFLPP_SRC_HASH");

pub fn aflpp_hash() -> &'static str {
    AFLPP_SRC_HASH
}

pub fn compile_aflpp_pcguard(cache_dir: impl AsRef<Path>, mut cc: ClangWrapper) -> io::Result<()> {
    let root = cache_dir.as_ref();
    let out = root.join("SanitizerCoveragePCGUARD.so");

    if out.is_file() {
        return Ok(());
    }

    let pcguard_files = &[
        "include/config.h",
        "include/types.h",
        "include/debug.h",
        "instrumentation/afl-llvm-common.cc",
        "instrumentation/afl-llvm-common.h",
        "instrumentation/SanitizerCoveragePCGUARD.so.cc",
    ];

    let mut archive = Archive::new(Decoder::new(AFLPP_SRC)?);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if pcguard_files.iter().any(|f| path == Path::new(f)) {
            entry.unpack_in(root)?;
        }
    }

    cc.cpp(true)
        .silence(true)
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
        .add_args(pcguard_files)
        .run()
        .unwrap();

    Ok(())
}
