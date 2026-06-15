//! LLVM compiler Wrapper from `LibAFL`

use crate::{
    CompilerWrapper, Configuration, Error, LlvmConfig, Result, ToolWrapper, dll_extension,
};
use core::str::FromStr;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// The supported LLVM passes
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LLVMPasses {
    //CmpLogIns,
    /// The `CmpLog` pass
    CmpLogRtn,
    /// The Autotoken pass
    AutoTokens,
    /// The Coverage Accouting (BB metric) pass
    CoverageAccounting,
    /// The dump cfg pass
    DumpCfg,
    #[cfg(unix)]
    /// The `CmpLog` Instruction pass
    CmpLogInstructions,
    /// Instrument caller for sancov coverage
    Ctx,
    /// Function logging
    FunctionLogging,
}

impl LLVMPasses {
    /// Gets the path of the LLVM pass
    #[must_use]
    pub fn path(&self, outdir: impl AsRef<Path>) -> PathBuf {
        let outdir = outdir.as_ref();
        match self {
            LLVMPasses::CmpLogRtn => {
                outdir.join(format!("cmplog-routines-pass.{}", dll_extension()))
            }
            LLVMPasses::AutoTokens => outdir.join(format!("autotokens-pass.{}", dll_extension())),
            LLVMPasses::CoverageAccounting => {
                outdir.join(format!("coverage-accounting-pass.{}", dll_extension()))
            }
            LLVMPasses::DumpCfg => outdir.join(format!("dump-cfg-pass.{}", dll_extension())),
            #[cfg(unix)]
            LLVMPasses::CmpLogInstructions => {
                outdir.join(format!("cmplog-instructions-pass.{}", dll_extension()))
            }
            LLVMPasses::Ctx => outdir.join(format!("ctx-pass.{}", dll_extension())),
            LLVMPasses::FunctionLogging => {
                outdir.join(format!("function-logging.{}", dll_extension()))
            }
        }
    }
}

/// Wrap Clang
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct ClangWrapper {
    llvm_config: LlvmConfig,
    wrapped_cc: PathBuf,
    wrapped_cxx: PathBuf,

    is_silent: bool,
    optimize: bool,

    name: String,
    is_cpp: bool,
    is_asm: bool,
    linking: bool,
    shared: bool,
    x_set: bool,
    bit_mode: u32,
    include_llvm_hdrs: bool,
    includes: Vec<PathBuf>,
    defines: Vec<(String, String)>,
    need_libaflmm_arg: bool,
    has_libaflmm_arg: bool,

    dir: Option<PathBuf>,
    output: Option<PathBuf>,
    configuration: crate::Configuration,
    ignoring_configurations: bool,
    parse_args_called: bool,
    base_args: Vec<String>,
    cc_args: Vec<String>,
    link_args: Vec<String>,
    passes: Vec<PathBuf>,
    passes_args: Vec<String>,
    passes_linking_args: Vec<String>,
}

#[expect(clippy::match_same_arms)] // for the linking = false wip for "shared"
impl ToolWrapper for ClangWrapper {
    #[expect(clippy::too_many_lines)]
    fn parse_args(&mut self, args: &[impl AsRef<str>]) -> Result<&'_ mut Self> {
        let mut new_args: Vec<String> = vec![];
        if args.is_empty() {
            return Err(Error::InvalidArguments(
                "The number of arguments cannot be 0".to_string(),
            ));
        }

        if self.parse_args_called {
            return Err(Error::Unknown(
                "ToolWrapper::parse_args cannot be called twice on the same instance".to_string(),
            ));
        }
        self.parse_args_called = true;

        if args.len() == 1 {
            return Err(Error::InvalidArguments(
                "LibAFL Tool wrapper - no commands specified. Use me as compiler.".to_string(),
            ));
        }

        self.name = args[0].as_ref().to_string();
        // Detect C++ compiler looking at the wrapper name
        self.is_cpp = if cfg!(windows) {
            self.is_cpp || self.name.ends_with("++.exe")
        } else {
            self.is_cpp || self.name.ends_with("++")
        };

        // Sancov flag
        // new_args.push("-fsanitize-coverage=trace-pc-guard".into());

        let mut linking = true;
        let mut shared = false;
        // Detect stray -v calls from ./configure scripts.
        if args.len() > 1 && args[1].as_ref() == "-v" {
            if args.len() == 2 {
                self.base_args.push(args[1].as_ref().into());
                return Ok(self);
            }
            linking = false;
        }

        // let mut suppress_linking = 0;
        let mut i = 1;
        while i < args.len() {
            let arg_as_path = Path::new(args[i].as_ref());

            if arg_as_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("s"))
            {
                self.is_asm = true;
            }

            match args[i].as_ref() {
                "--libaflmm-no-link" => {
                    // suppress_linking += 1;
                    self.has_libaflmm_arg = true;
                    i += 1;
                    continue;
                }
                "--libafl" => {
                    // suppress_linking += 1337;
                    self.has_libaflmm_arg = true;
                    i += 1;
                    continue;
                }
                "-fsanitize=fuzzer-no-link" => {
                    // suppress_linking += 1;
                    self.has_libaflmm_arg = true;
                    i += 1;
                    continue;
                }
                "-fsanitize=fuzzer" => {
                    // suppress_linking += 1337;
                    self.has_libaflmm_arg = true;
                    i += 1;
                    continue;
                }
                "-Wl,-z,defs" | "-Wl,--no-undefined" | "--no-undefined" => {
                    i += 1;
                    continue;
                }
                "-z" | "-Wl,-z"
                    if i + 1 < args.len()
                        && (args[i + 1].as_ref() == "defs"
                            || args[i + 1].as_ref() == "-Wl,defs") =>
                {
                    i += 2;
                    continue;
                }
                "--libaflmm-ignore-configurations" | "-print-prog-name=ld" => {
                    self.ignoring_configurations = true;
                    i += 1;
                    continue;
                }
                "--libafl-configurations" if i + 1 < args.len() => {
                    self.configuration =
                        crate::Configuration::from_str(args[i + 1].as_ref()).unwrap();
                    i += 2;
                    continue;
                }
                "-o" if i + 1 < args.len() => {
                    self.output = Some(PathBuf::from(args[i + 1].as_ref()));
                    i += 2;
                    continue;
                }
                "-x" => self.x_set = true,
                "-m32" => self.bit_mode = 32,
                "-m64" => self.bit_mode = 64,
                "-c" | "-S" | "-E" => linking = false,
                "-shared" => {
                    linking = false;
                    shared = true;
                } // TODO dynamic list?
                _ => (),
            }
            new_args.push(args[i].as_ref().to_string());
            i += 1;
        }

        // if linking
        //     && (suppress_linking > 0 || (self.has_libaflmm_arg && suppress_linking == 0))
        //     && suppress_linking < 1337
        // {
        //     linking = false;
        //     new_args.push(
        //         PathBuf::from(env!("OUT_DIR"))
        //             .join(format!("{LIB_PREFIX}no-link-rt.{LIB_EXT}"))
        //             .into_os_string()
        //             .into_string()
        //             .unwrap(),
        //     );
        // }

        self.linking = linking;
        self.shared = shared;

        new_args.push("-g".into());
        if self.optimize {
            new_args.push("-O3".into());
            new_args.push("-funroll-loops".into());
        }

        // Fuzzing define common among tools
        new_args.push("-DFUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION=1".into());

        // Libraries needed by libafl on Windows
        #[cfg(windows)]
        if linking {
            new_args.push("-lws2_32".into());
            new_args.push("-lBcrypt".into());
            new_args.push("-lAdvapi32".into());
        }
        // required by timer API (timer_create, timer_settime)
        #[cfg(target_os = "linux")]
        if linking {
            new_args.push("-lrt".into());
        }
        // `MacOS` has odd linker behavior sometimes
        #[cfg(target_vendor = "apple")]
        if linking || shared {
            new_args.push("-undefined".into());
            new_args.push("dynamic_lookup".into());
        }

        self.base_args.extend(new_args);

        Ok(self)
    }

    fn add_arg(&mut self, arg: impl AsRef<str>) -> &'_ mut Self {
        self.base_args.push(arg.as_ref().to_string());
        self
    }

    fn set_dir(&mut self, dir: impl AsRef<Path>) -> &'_ mut Self {
        self.dir = Some(dir.as_ref().to_path_buf());
        self
    }

    fn dir(&self) -> Option<&Path> {
        self.dir.as_ref().map(|p| p.as_path())
    }

    fn set_configuration(&mut self, configuration: crate::Configuration) -> &'_ mut Self {
        self.configuration = configuration;
        self
    }

    fn configuration(&self) -> Result<crate::Configuration> {
        let config = self.configuration.clone();
        Ok(config)
    }

    fn ignore_configurations(&self) -> Result<bool> {
        Ok(self.ignoring_configurations)
    }

    fn command(&mut self) -> Result<Vec<String>> {
        self.command_for_configuration(crate::Configuration::Default)
    }

    #[expect(clippy::too_many_lines)]
    fn command_for_configuration(
        &mut self,
        configuration: crate::Configuration,
    ) -> Result<Vec<String>> {
        let mut args = vec![];
        // let mut use_pass = false;

        if self.is_cpp {
            args.push(self.wrapped_cxx.to_str().unwrap().to_string());
        } else {
            args.push(self.wrapped_cc.to_str().unwrap().to_string());
        }

        let base_args = self
            .base_args
            .iter()
            .map(|r| {
                let arg_as_path = PathBuf::from(r);
                if r.ends_with('.') {
                    r.clone()
                } else {
                    if let Some(extension) = arg_as_path.extension() {
                        let extension = extension.to_str().unwrap();
                        let extension_lowercase = extension.to_lowercase();
                        match &extension_lowercase[..] {
                            "a" | "la" | "pch" => configuration.replace_extension(&arg_as_path),
                            _ => arg_as_path,
                        }
                    } else {
                        arg_as_path
                    }
                    .into_os_string()
                    .into_string()
                    .unwrap()
                }
            })
            .collect::<Vec<_>>();

        if let crate::Configuration::Default = configuration {
            if let Some(output) = self.output.clone() {
                let output = configuration.replace_extension(&output);
                let new_filename = output.into_os_string().into_string().unwrap();
                args.push("-o".to_string());
                args.push(new_filename);
            }
        } else if let Some(output) = self.output.clone() {
            let output = configuration.replace_extension(&output);
            let new_filename = output.into_os_string().into_string().unwrap();
            args.push("-o".to_string());
            args.push(new_filename);
        } else {
            // No output specified, we need to rewrite the single .c file's name into a -o
            // argument.
            for arg in &base_args {
                let arg_as_path = PathBuf::from(arg);
                if !arg.ends_with('.')
                    && !arg.starts_with('-')
                    && let Some(extension) = arg_as_path.extension()
                {
                    let extension = extension.to_str().unwrap();
                    let extension_lowercase = extension.to_lowercase();
                    match &extension_lowercase[..] {
                        "c" | "cc" | "cxx" | "cpp" => {
                            args.push("-o".to_string());
                            args.push(if self.linking {
                                configuration
                                    .replace_extension(&PathBuf::from("a.out"))
                                    .into_os_string()
                                    .into_string()
                                    .unwrap()
                            } else {
                                let mut result = configuration.replace_extension(&arg_as_path);
                                result.set_extension("o");
                                result.into_os_string().into_string().unwrap()
                            });
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        args.extend_from_slice(base_args.as_slice());

        args.extend_from_slice(&configuration.to_flags()?);

        if self.need_libaflmm_arg && !self.has_libaflmm_arg {
            return Ok(args);
        }

        if !self.is_asm && !self.passes.is_empty() {
            for passes_arg in &self.passes_args {
                args.push("-mllvm".into());
                args.push(passes_arg.into());
            }
        }
        if self.linking {
            if self.x_set {
                args.push("-x".into());
                args.push("none".into());
            }

            args.extend_from_slice(self.link_args.as_slice());

            // if use_pass {
            //     args.extend_from_slice(self.passes_linking_args.as_slice());
            // }

            if cfg!(unix) {
                args.push("-pthread".into());
                args.push("-ldl".into());
                args.push("-lm".into());
            }
        } else {
            args.extend_from_slice(self.cc_args.as_slice());
        }

        Ok(args)
    }

    fn is_linking(&self) -> bool {
        self.linking
    }

    fn filter(&self, args: &mut Vec<String>) {
        let blocklist = ["-Werror=unused-command-line-argument", "-Werror"];
        for item in blocklist {
            args.retain(|x| x.clone() != item);
        }
    }

    fn silence(&mut self, value: bool) -> &'_ mut Self {
        self.is_silent = value;
        self
    }

    fn is_silent(&self) -> bool {
        self.is_silent
    }

    fn run(&mut self) -> Result<Output> {
        let configuration = if self.ignore_configurations()? {
            Configuration::Default
        } else {
            self.configuration()?
        };

        let mut args = self.command_for_configuration(configuration)?;
        self.filter(&mut args);

        let mut cmd = Command::new(&args[0]);

        cmd.args(&args[1..]);

        if self.include_llvm_hdrs {
            let headers = self.llvm_config.include_dir()?;

            cmd.arg("-I");
            cmd.arg(headers);
        }

        for incl in &self.includes {
            cmd.arg("-I");
            cmd.arg(incl);
        }

        for (def, value) in &self.defines {
            cmd.arg(format!("-D{def}={value}"));
        }

        for pass in &self.passes {
            // https://github.com/llvm/llvm-project/issues/56137
            // Need this -Xclang -load -Xclang -<pass>.so thing even with the new PM
            // to pass the arguments to LLVM Passes
            cmd.arg("-Xclang");
            cmd.arg("-load");
            cmd.arg("-Xclang");
            cmd.arg(pass.clone());
            cmd.arg("-Xclang");
            cmd.arg(format!("-fpass-plugin={}", pass.display()));
        }

        if let Some(dir) = self.dir() {
            cmd.current_dir(dir);
        }

        if !self.is_silent() {
            let args: Vec<&OsStr> = cmd.get_args().collect();
            dbg!(args);
        }

        if cmd.get_args().count() == 0 {
            return Err(Error::InvalidArguments(
                "The number of arguments cannot be 0".into(),
            ));
        }

        let output = cmd.output()?;

        if !self.is_silent() {
            dbg!(output.status);
        }

        Ok(output)
    }
}

impl CompilerWrapper for ClangWrapper {
    fn add_cc_arg(&mut self, arg: impl AsRef<str>) -> &'_ mut Self {
        self.cc_args.push(arg.as_ref().to_string());
        self
    }

    fn add_link_arg(&mut self, arg: impl AsRef<str>) -> &'_ mut Self {
        self.link_args.push(arg.as_ref().to_string());
        self
    }

    fn add_include(&mut self, include_dir: impl AsRef<Path>) -> &'_ mut Self {
        self.includes.push(include_dir.as_ref().to_path_buf());
        self
    }

    fn define(&mut self, define: impl AsRef<str>, value: impl AsRef<str>) -> &'_ mut Self {
        self.defines
            .push((define.as_ref().to_string(), value.as_ref().to_string()));
        self
    }

    fn link_staticlib(&mut self, lib: impl AsRef<Path>) -> &'_ mut Self {
        let lib_str = lib.as_ref().as_os_str().to_str().unwrap();

        if cfg!(unix) {
            if cfg!(target_vendor = "apple") {
                // Same as --whole-archive on linux
                // Without this option, the linker picks the first symbols it finds and does not care if it's a weak or a strong symbol
                // See: <https://stackoverflow.com/questions/13089166/how-to-make-gcc-link-strong-symbol-in-static-library-to-overwrite-weak-symbol>
                self.add_link_arg("-Wl,-force_load").add_link_arg(lib_str)
            } else {
                self.add_link_arg("-Wl,--whole-archive")
                    .add_link_arg(lib_str)
                    .add_link_arg("-Wl,--no-whole-archive")
            }
        } else {
            self.add_link_arg(format!("-Wl,-wholearchive:{lib_str}"))
        }
    }
}

impl TryFrom<&LlvmConfig> for ClangWrapper {
    type Error = Error;

    fn try_from(llvm_config: &LlvmConfig) -> Result<Self> {
        Self::new(llvm_config.clone())
    }
}

impl ClangWrapper {
    /// Create a new Clang Wrapper
    #[must_use]
    pub fn new(llvm_config: LlvmConfig) -> Result<Self> {
        let wrapped_cc = llvm_config.clang()?;
        let wrapped_cxx = llvm_config.clangpp()?;

        Ok(Self {
            optimize: true,
            wrapped_cc,
            wrapped_cxx,
            name: String::new(),
            is_cpp: false,
            is_asm: false,
            linking: false,
            shared: false,
            x_set: false,
            bit_mode: 0,
            need_libaflmm_arg: false,
            has_libaflmm_arg: false,
            include_llvm_hdrs: false,
            llvm_config,
            includes: vec![],
            defines: vec![],
            dir: None,
            output: None,
            configuration: crate::Configuration::Default,
            ignoring_configurations: false,
            parse_args_called: false,
            base_args: vec![],
            cc_args: vec![],
            link_args: vec![],
            passes: vec![],
            passes_args: vec![],
            passes_linking_args: vec![],
            is_silent: false,
        })
    }

    /// create a new [`Self`] from another one.
    /// The resulting [`Self`] has the same defaults as
    /// a normal default, but with the same llvm bin paths.
    pub fn renew(&self) -> Result<Self> {
        Self::new(self.llvm_config.clone())
    }

    /// Unique string identifying clang
    pub fn version(&self) -> Result<String> {
        let res = self
            .renew()?
            .silence(true)
            .add_arg("--version")
            .run()
            .unwrap();
        Ok(String::from_utf8(res.stdout).unwrap())
    }

    pub fn output(&mut self, out: impl AsRef<Path>) -> &'_ mut Self {
        self.output = Some(out.as_ref().to_path_buf());
        self
    }

    /// Disable optimizations, call this before calling `parse_args`
    pub fn dont_optimize(&mut self) -> &'_ mut Self {
        self.optimize = false;
        self
    }

    /// Set cpp mode, call this before calling `parse_args`
    pub fn cpp(&mut self, value: bool) -> &'_ mut Self {
        self.is_cpp = value;
        self
    }

    // /// Add LLVM pass
    // pub fn add_pass(&mut self, pass: LLVMPasses) -> &'_ mut Self {
    //     self.passes.push(pass.path());
    //     self
    // }

    /// Add a pre-compiled LLVM pass .so file to the pipeline
    ///
    /// Use [`compile_custom_pass`] first to build the `.so` from the source code first
    pub fn add_pass<P: AsRef<Path>>(&mut self, pass_so: P) -> Result<&'_ mut Self> {
        let pass = pass_so.as_ref();
        if !pass.is_file() {
            return Err(Error::InvalidArguments(format!(
                "Not a file: {}",
                pass.display()
            )));
        }

        if pass.extension().unwrap() != OsStr::new("so") {
            return Err(Error::InvalidArguments(format!(
                "Not a .so file: {}",
                pass.display()
            )));
        }

        self.passes.push(pass.to_path_buf());

        Ok(self)
    }

    /// Add LLVM pass arguments
    pub fn add_passes_arg<S>(&mut self, arg: S) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        self.passes_args.push(arg.as_ref().to_string());
        self
    }

    /// Add arguments for LLVM passes during linking. For example, ngram needs -lm
    pub fn add_passes_linking_arg<S>(&mut self, arg: S) -> &'_ mut Self
    where
        S: AsRef<str>,
    {
        self.passes_linking_args.push(arg.as_ref().to_string());
        self
    }

    /// Set if linking
    pub fn linking(&mut self, value: bool) -> &'_ mut Self {
        self.linking = value;
        self
    }

    /// Set if it needs the --libafl arg to add the custom arguments to clang
    pub fn need_libaflmm_arg(&mut self, value: bool) -> &'_ mut Self {
        self.need_libaflmm_arg = value;
        self
    }

    pub fn include_llvm_headers(&mut self, value: bool) -> &'_ mut Self {
        self.include_llvm_hdrs = value;
        self
    }
}

// /// Compile a given LLVM pass source code into a shared object (and return it)
// ///
// /// this one is basically just the same as the `build_pass` in build.rs but just but the pass on-demand
// pub fn compile_custom_pass(src: &Path) -> Result<PathBuf, Error> {
//     let version = self.
//
//     let stem = src.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
//         Error::InvalidArguments(format!("Invalid pass source path: {}", src.display()))
//     })?;
//
//     let cache_dir = PathBuf::from(OUT_DIR).join("custom-passes");
//     fs::create_dir_all(&cache_dir).map_err(Error::Io)?;
//     let ext = dll_extension();
//     let out_path = cache_dir.join(format!("{stem}-llvm{version}.{ext}"));
//     if out_path.exists() {
//         return Ok(out_path);
//     }
//
//     #[cfg(unix)]
//     let status = Command::new(CLANGXX_PATH)
//         .arg("-v")
//         .arg(format!("--target={HOST_TARGET}"))
//         .args(LLVM_CXXFLAGS)
//         .arg(src)
//         .args(LLVM_LDFLAGS)
//         .arg("-o")
//         .arg(&out_path)
//         .status()
//         .map_err(Error::Io)?;
//
//     #[cfg(windows)]
//     let status = Command::new(Path::new(LLVM_BINDIR).join("clang-cl.exe"))
//         .arg("-v")
//         .arg(format!("--target={HOST_TARGET}"))
//         .args(LLVM_CXXFLAGS)
//         .arg(src)
//         .arg("/link")
//         .args(LLVM_LDFLAGS)
//         .arg(format!("/OUT:{}", out_path.display()))
//         .status()
//         .map_err(Error::Io)?;
//
//     #[cfg(not(any(unix, windows)))]
//     return Err(Error::Unknown(
//         "Custom pass compilation is only supported on unix or windows".to_string(),
//     ));
//
//     #[cfg(any(unix, windows))]
//     {
//         if !status.success() {
//             let _ = fs::remove_file(&out_path);
//             return Err(Error::Unknown(format!(
//                 "Failed to compile custom pass {}: exit status {status}",
//                 src.display()
//             )));
//         }
//         Ok(out_path)
//     }
// }
//
