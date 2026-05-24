use crate::{
    Result,
    arch::{GuestReg, Regs},
    emu::{Emulator, EmulatorDriverResult},
    qemu::{Qemu, QemuRWError},
    sync_exit::ExitArgs,
};
use enum_map::EnumMap;
use std::{
    ffi::c_uint,
    fmt::{self, Debug, Display, Formatter},
};

#[cfg(not(feature = "nyx"))]
pub mod lqemu;
#[cfg(all(not(feature = "nyx"), feature = "systemmode"))]
pub use lqemu::SetMapCommand;
#[cfg(not(feature = "nyx"))]
pub use lqemu::{
    AddressAllowCommand, EndCommand, LoadCommand, LqemuCommandManager, LqprintfCommand,
    SaveCommand, StartCommand, TestCommand, VersionCommand,
};

#[cfg(feature = "nyx")]
pub mod nyx;
#[cfg(feature = "nyx")]
pub use nyx::{
    AcquireCommand, GetHostConfigCommand, GetPayloadCommand, NextPayloadCommand, NyxCommandManager,
    PanicCommand, PrintfCommand, RangeSubmitCommand, ReleaseCommand, SetAgentConfigCommand,
    SubmitCR3Command, SubmitPanicCommand, UserAbortCommand,
};

#[cfg(not(feature = "nyx"))]
pub type StdCommandManager = LqemuCommandManager;
#[cfg(feature = "nyx")]
pub type StdCommandManager = NyxCommandManager;

#[macro_export]
macro_rules! define_std_command_manager_bound {
    ($name:ident, $input_bound:ty, [$($command:ty),+], [$($native_command_parser:ty),+]) => {
        define_std_command_manager_inner!($name, ($input_bound,), [$($command),+], [$($native_command_parser),+]);
    };
}

#[macro_export]
macro_rules! define_std_command_manager_type {
    ($name:ident, $input_type:ty, [$($command:ty),+], [$($native_command_parser:ty),+]) => {
        define_std_command_manager_inner!($name, (), [$($command),+], [$($native_command_parser),+], $input_type);
    };
}

#[macro_export]
macro_rules! define_std_command_manager_inner {
    ($name:ident, ($($input_bound:ty,)?), [$($command:ty),+], [$($native_command_parser:ty),+]$(, $input_type:ty)?) => {
        paste! {
            pub use [< $name:snake >]::$name;

            mod [< $name:snake >] {
                use super::*;

                use std::{
                    fmt,
                    fmt::{Debug, Formatter},
                };
                use enum_map::EnumMap;
                use $crate::{
                    command::{CommandManager, CommandError, NativeCommandParser, Command},
                    arch::get_exit_arch_regs,
                    sync_exit::ExitArgs,
                    emu::EmulatorDriverResult,
                    qemu::Qemu,
                    arch::Regs,
                    Result,

                };
                use std::ffi::c_uint;

                pub struct $name {
                    has_started: bool,
                }

                impl Clone for $name {
                    fn clone(&self) -> Self {
                        Self {
                            has_started: self.has_started,
                        }
                    }
                }

                impl Debug for $name {
                    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                        write!(f, "{} (has started? {:?})", stringify!($name), self.has_started)
                    }
                }

                impl Default for $name {
                    fn default() -> Self {
                        Self {
                            has_started: false,
                        }
                    }
                }

                impl CommandManager for $name {
                    type Commands = [<$name Commands>];

                    fn start(&mut self) -> bool {
                        let tmp = self.has_started;
                        self.has_started = true;
                        tmp
                    }

                    fn has_started(&self) -> bool {
                        self.has_started
                    }

                    #[deny(unreachable_patterns)]
                    fn parse(&self, qemu: Qemu) -> Result<Self::Commands> {
                        let arch_regs_map: &'static EnumMap<ExitArgs, Regs> = get_exit_arch_regs();
                        let cmd_id = qemu.read_reg(arch_regs_map[ExitArgs::Cmd])? as c_uint;

                        match cmd_id {
                            // <StartPhysCommandParser as NativeCommandParser<S>>::COMMAND_ID => Ok(StdCommandManagerCommands::StartPhysCommandParserCmd(<StartPhysCommandParser as NativeCommandParser<S>>::parse(qemu, arch_regs_map)?)),
                            $(<$native_command_parser as NativeCommandParser>::COMMAND_ID => Ok(<$native_command_parser as NativeCommandParser>::parse(qemu, arch_regs_map)?.into())),+,
                            _ => Err(CommandError::UnknownCommand(cmd_id.into()).into()),
                        }
                    }
                }

                #[derive(Clone, Debug)]
                #[expect(clippy::enum_variant_names)]
                pub enum [<$name Commands>]
                {
                    // StartPhysCommand(StartPhysCommand)
                    $($command($command)),+,
                }

                impl Command for [<$name Commands>] {
                    fn usable_at_runtime(&self) -> bool {
                        match self {
                            $([<$name Commands>]::$command(cmd) => <$command as Command>::usable_at_runtime(cmd)),+
                        }
                    }

                    fn run<EMU>(&self,
                        emu: &mut EMU,
                        ret_reg: Option<Regs>
                    ) -> Result<Option<EmulatorDriverResult<EMU::Command>>>
                    where
                        EMU: Emulator
                    {
                        match self {
                            $([<$name Commands>]::$command(cmd) => cmd.run(emu, ret_reg)),+
                        }
                    }
                }

                $(
                    impl From<$command> for [<$name Commands>] {
                        fn from(cmd: $command) -> [<$name Commands>] {
                            [<$name Commands>]::$command(cmd)
                        }
                    }
                )+
            }
        }
    };
}

pub trait NativeCommandParser {
    type OutputCommand: Command;

    const COMMAND_ID: c_uint;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand>;
}

pub trait CommandManager: Sized + Debug {
    type Commands: Command;

    /// Returns whether the command manager has been started already.
    fn has_started(&self) -> bool;

    /// Mark the command manager as started.
    /// it should return if it has been started before or not.
    fn start(&mut self) -> bool;

    fn parse(&self, qemu: Qemu) -> Result<Self::Commands>;
}

pub trait Command: Clone + Debug {
    /// Used to know whether the command can be run during a backdoor, or if it is necessary to go out of
    /// the QEMU VM to run the command.
    // TODO: Use const when stabilized
    fn usable_at_runtime(&self) -> bool;

    /// Command handler.
    ///     - `ret_reg`: The register in which the guest return value should be written, if any.
    /// Returns
    ///     - `InnerHandlerResult`: How the high-level handler should behave
    fn run<EMU: Emulator>(
        &self,
        emu: &mut EMU,
        ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorDriverResult<<EMU as Emulator>::Command>>>;
}

#[derive(Debug, Clone)]
pub enum CommandError {
    UnknownCommand(GuestReg),
    RWError(QemuRWError),
    VersionDifference(u64, u64),
    TestDifference(GuestReg, GuestReg), // received, expected
    InvalidParameters,
    StartedTwice,
    EndBeforeStart,
    WrongUsage,
}

#[derive(Debug, Copy, Clone)]
pub struct NopCommandManager;
impl CommandManager for NopCommandManager {
    type Commands = NopCommand;

    fn has_started(&self) -> bool {
        false
    }

    fn start(&mut self) -> bool {
        false
    }

    fn parse(&self, _qemu: Qemu) -> Result<Self::Commands> {
        Ok(NopCommand)
    }
}

impl From<QemuRWError> for CommandError {
    fn from(error: QemuRWError) -> Self {
        CommandError::RWError(error)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NopCommand;

impl Display for NopCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "NopCommand")
    }
}

impl Command for NopCommand {
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU: Emulator>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorDriverResult<<EMU as Emulator>::Command>>> {
        Ok(None)
    }
}
