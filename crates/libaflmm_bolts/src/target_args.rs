//! Shared implementation of afl style arguments

use alloc::{borrow::ToOwned, vec::Vec};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use crate::fs::InputFile;

/// How to deliver input to an external program
/// `StdIn`: The target reads from stdin
/// `File`: The target reads from the specified [`InputFile`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputLocation {
    /// Mutate a commandline argument to deliver an input
    Arg {
        /// The offset of the argument to mutate
        argnum: usize,
    },
    /// Deliver input via `StdIn`
    StdIn {
        /// The alternative input file
        input_file: Option<InputFile>,
    },
    /// Deliver the input via the specified [`InputFile`]
    /// You can use [`InputFile::create`] with [`crate::fs::INPUTFILE_STD`] to use a default filename.
    File {
        /// The file to write input to. The target should read input from this location.
        out_file: InputFile,
    },
}

impl Default for InputLocation {
    fn default() -> Self {
        Self::StdIn { input_file: None }
    }
}

/// The shared inner structs of trait [`StdTargetArgs`]
#[derive(Debug, Clone, Default)]
pub struct StdTargetArgsInner {
    /// Program arguments
    pub arguments: Vec<OsString>,
    /// Program main program
    pub program: Option<OsString>,
    /// Input location, might be stdin or file or cli arg
    pub input_location: InputLocation,
    /// Program environments
    pub envs: Vec<(OsString, OsString)>,
}

/// The main implementation trait of afl style arguments handling
pub trait StdTargetArgs: Sized {
    /// Get inner common arguments
    fn inner(&self) -> &StdTargetArgsInner;

    /// Get mutable inner common arguments
    fn inner_mut(&mut self) -> &mut StdTargetArgsInner;

    /// Adds an environmental var to the harness's commandline
    #[must_use]
    fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner_mut()
            .envs
            .push((key.as_ref().to_owned(), val.as_ref().to_owned()));
        self
    }

    /// Adds environmental vars to the harness's commandline
    #[must_use]
    fn envs<IT, K, V>(mut self, vars: IT) -> Self
    where
        IT: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let mut res = vec![];
        for (ref key, ref val) in vars {
            res.push((key.as_ref().to_owned(), val.as_ref().to_owned()));
        }
        self.inner_mut().envs.append(&mut res);
        self
    }

    /// If use stdin
    #[must_use]
    fn use_stdin(&self) -> bool {
        matches!(
            &self.inner().input_location,
            InputLocation::StdIn { input_file: _ }
        )
    }

    /// Set input
    #[must_use]
    fn input(mut self, input: InputLocation) -> Self {
        self.inner_mut().input_location = input;
        self
    }

    /// Sets the input mode to [`InputLocation::Arg`] and uses the current arg offset as `argnum`.
    /// During execution, at input will be provided _as argument_ at this position.
    /// Use [`Self::arg_input_file`] if you want to provide the input as a file instead.
    #[must_use]
    fn arg_input_arg(mut self) -> Self {
        let argnum = self.inner().arguments.len();
        self = self.input(InputLocation::Arg { argnum });
        // Placeholder arg that gets replaced with the input name later.
        self = self.arg("PLACEHOLDER");
        self
    }

    /// Place the input at this position and set the filename for the input.
    ///
    /// Note: If you use this, you should ensure that there is only one instance using this
    /// file at any given time.
    #[must_use]
    fn arg_input_file(self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut moved = self.arg(&path);
        assert!(
            match &moved.inner().input_location {
                InputLocation::File { out_file } => out_file.path == path,
                InputLocation::StdIn { input_file } =>
                    input_file.as_ref().is_none_or(|of| of.path == path),
                InputLocation::Arg { argnum: _ } => false,
            },
            "Already specified an input file under a different name. This is not supported"
        );
        let out_file = InputFile::create(path).unwrap();
        moved = moved.input(InputLocation::File { out_file });
        moved
    }

    /// The harness
    #[must_use]
    fn program<O>(mut self, program: O) -> Self
    where
        O: AsRef<OsStr>,
    {
        self.inner_mut().program = Some(program.as_ref().to_owned());
        self
    }

    /// Adds an argument to the harness's commandline
    #[must_use]
    fn arg<O>(mut self, arg: O) -> Self
    where
        O: AsRef<OsStr>,
    {
        self.inner_mut().arguments.push(arg.as_ref().to_owned());
        self
    }

    /// Adds arguments to the harness's commandline
    #[must_use]
    fn args<IT, O>(mut self, args: IT) -> Self
    where
        IT: IntoIterator<Item = O>,
        O: AsRef<OsStr>,
    {
        let mut res = vec![];
        for arg in args {
            res.push(arg.as_ref().to_owned());
        }
        self.inner_mut().arguments.append(&mut res);
        self
    }
}
