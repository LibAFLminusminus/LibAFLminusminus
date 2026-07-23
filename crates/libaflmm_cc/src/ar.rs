//! Ar Wrapper from `LibAFL`
// pass to e.g. cmake with -DCMAKE_AR=/path/to/fuzzer/target/release/libaflmm_ar

use core::str::FromStr;
use std::path::{Path, PathBuf};

use crate::{Error, LlvmConfig, Result, ToolWrapper};

/// Wrap Clang
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct ArWrapper {
    is_silent: bool,

    name: String,
    linking: bool,
    need_libaflmm_arg: bool,
    has_libaflmm_arg: bool,

    bin: PathBuf,
    dir: Option<PathBuf>,
    configuration: crate::Configuration,
    parse_args_called: bool,
    base_args: Vec<String>,
}

#[expect(clippy::match_same_arms)] // for the linking = false wip for "shared"
impl ToolWrapper for ArWrapper {
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

        let mut linking = true;
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
            match args[i].as_ref() {
                "--libafl-no-link" => {
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
                "--libafl-configurations" if i + 1 < args.len() => {
                    self.configuration =
                        crate::Configuration::from_str(args[i + 1].as_ref()).unwrap();
                    i += 2;
                    continue;
                }
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

        // Libraries needed by libafl on Windows
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
        Ok(false)
    }

    fn command(&mut self) -> Result<Vec<String>> {
        self.command_for_configuration(crate::Configuration::Default)
    }

    fn command_for_configuration(
        &mut self,
        configuration: crate::Configuration,
    ) -> Result<Vec<String>> {
        let mut args = vec![];

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
                            "o" | "lo" | "a" | "la" | "so" | "ao" | "c.o" | "pch" => {
                                configuration.replace_extension(&arg_as_path)
                            }
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

        args.push(self.bin.as_path().to_str().unwrap().to_string());

        args.extend_from_slice(base_args.as_slice());

        if self.need_libaflmm_arg && !self.has_libaflmm_arg {
            return Ok(args);
        }

        Ok(args)
    }

    fn is_linking(&self) -> bool {
        self.linking
    }

    fn filter(&self, _args: &mut Vec<String>) {}

    fn silence(&mut self, value: bool) -> &'_ mut Self {
        self.is_silent = value;
        self
    }

    fn is_silent(&self) -> bool {
        self.is_silent
    }
}

impl ArWrapper {
    /// Create a new Clang Wrapper
    #[must_use]
    pub fn new(llvm_config: &LlvmConfig) -> Result<Self> {
        let bin = llvm_config.tool("ar")?;

        Ok(Self {
            name: String::new(),
            linking: false,
            need_libaflmm_arg: false,
            has_libaflmm_arg: false,
            bin,
            dir: None,
            configuration: crate::Configuration::Default,
            parse_args_called: false,
            base_args: vec![],
            is_silent: false,
        })
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
}
