use crate::{
    Result,
    arch::{GuestReg, Regs},
    emu::{Emulator, EmulatorRunResult, InputWriter, NopInputWriter},
    qemu::{Qemu, QemuRWError},
    sync_exit::ExitArgs,
};
use enum_map::EnumMap;
use std::{
    ffi::c_uint,
    fmt::{self, Debug, Display, Formatter},
    result,
};
use thiserror::Error;

#[cfg(not(feature = "nyx"))]
pub mod lqemu;
#[cfg(all(not(feature = "nyx"), feature = "systemmode"))]
pub use lqemu::SetMapCommand;
#[cfg(not(feature = "nyx"))]
pub use lqemu::{
    AddressAllowCommand, EndCommand, LoadCommand, LqemuCommandManager, LqemuCommands,
    LqprintfCommand, SaveCommand, StartCommand, TestCommand, VersionCommand,
};

#[cfg(feature = "nyx")]
pub mod nyx;
#[cfg(feature = "nyx")]
pub use nyx::{
    AcquireCommand, GetHostConfigCommand, GetPayloadCommand, NextPayloadCommand, NyxCommandManager,
    NyxCommands, PanicCommand, PrintfCommand, RangeSubmitCommand, ReleaseCommand,
    SetAgentConfigCommand, SubmitCR3Command, SubmitPanicCommand, UserAbortCommand,
};

#[cfg(not(feature = "nyx"))]
pub type StdCommandManager = LqemuCommandManager;
#[cfg(not(feature = "nyx"))]
pub type StdCommands = LqemuCommands;

#[cfg(feature = "nyx")]
pub type StdCommandManager = NyxCommandManager;
#[cfg(feature = "nyx")]
pub type StdCommands = NyxCommands;

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
            pub use [< $name:snake >]::{[<$name CommandManager>], [<$name Commands>]};

            mod [< $name:snake >] {
                use super::*;

                use std::{
                    fmt,
                    fmt::{Debug, Formatter},
                };
                use core::result;
                use enum_map::EnumMap;
                use $crate::{
                    command::{CommandManager, CommandError, NativeCommandParser, Command},
                    arch::get_exit_arch_regs,
                    sync_exit::ExitArgs,
                    emu::{EmulatorRunResult, StdInputWriter},
                    qemu::Qemu,
                    arch::Regs,
                    Result,

                };
                use std::ffi::c_uint;
                use libaflmm::{inputs::Input, states::State};

                pub struct [<$name CommandManager>] {
                    has_started: bool,
                }

                impl Clone for [<$name CommandManager>] {
                    fn clone(&self) -> Self {
                        Self {
                            has_started: self.has_started,
                        }
                    }
                }

                impl Debug for [<$name CommandManager>] {
                    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                        write!(f, "{} (has started? {:?})", stringify!($name), self.has_started)
                    }
                }

                impl Default for [<$name CommandManager>] {
                    fn default() -> Self {
                        Self {
                            has_started: false,
                        }
                    }
                }

                impl<I, S> CommandManager<I, S> for [<$name CommandManager>]
                where
                    I: Input + Unpin,
                    S: State<Input = I> + Unpin,
                {
                    type Commands = [<$name Commands>];
                    type InputWriter = StdInputWriter;

                    fn start(&mut self) -> bool {
                        let tmp = self.has_started;
                        self.has_started = true;
                        tmp
                    }

                    fn has_started(&self) -> bool {
                        self.has_started
                    }

                    #[deny(unreachable_patterns)]
                    fn parse(&self, qemu: Qemu) -> result::Result<Self::Commands, CommandError> {
                        let arch_regs_map: &'static EnumMap<ExitArgs, Regs> = get_exit_arch_regs();
                        let cmd_id = qemu.read_reg(arch_regs_map[ExitArgs::Cmd])? as c_uint;

                        match cmd_id {
                            // <StartPhysCommandParser as NativeCommandParser<S>>::COMMAND_ID => Ok(StdCommandManagerCommands::StartPhysCommandParserCmd(<StartPhysCommandParser as NativeCommandParser<S>>::parse(qemu, arch_regs_map)?)),
                            $(<$native_command_parser as NativeCommandParser>::COMMAND_ID => Ok(<$native_command_parser as NativeCommandParser>::parse(qemu, arch_regs_map)?.into())),+,
                            _ => Err(CommandError::UnknownCommand(cmd_id.into())),
                        }
                    }
                }

                #[derive(Clone, Debug)]
                pub enum [<$name Commands>]
                {
                    // StartPhysCommand(StartPhysCommand)
                    $($command([<$command Command>])),+,
                }

                impl<I, S> Command<[<$name CommandManager>], I, S> for [<$name Commands>]
                where
                    I: Input + Unpin,
                    S: State<Input = I> + Unpin,
                {
                    fn usable_at_runtime(&self) -> bool {
                        match self {
                            $([<$name Commands>]::$command(cmd) => <[<$command Command>] as Command<[<$name CommandManager>], I, S>>::usable_at_runtime(cmd)),+
                        }
                    }

                    fn run<EMU>(&self,
                        emu: &mut EMU,
                        ret_reg: Option<Regs>
                    ) -> Result<Option<EmulatorRunResult>>
                    where
                        EMU: Emulator<I, S, CommandManager = [<$name CommandManager>]>
                    {
                        match self {
                            $([<$name Commands>]::$command(cmd) => cmd.run(emu, ret_reg)),+
                        }
                    }
                }

                $(
                    impl From<[<$command Command>]> for [<$name Commands>] {
                        fn from(cmd: [<$command Command>]) -> [<$name Commands>] {
                            [<$name Commands>]::$command(cmd)
                        }
                    }
                )+
            }
        }
    };
}

pub trait NativeCommandParser {
    type OutputCommand;

    const COMMAND_ID: c_uint;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError>;
}

pub trait CommandManager<I, S>: Sized + Debug {
    type Commands: Command<Self, I, S>;
    type InputWriter: InputWriter<I, S>;

    /// Returns whether the command manager has been started already.
    fn has_started(&self) -> bool;

    /// Mark the command manager as started.
    /// it should return if it has been started before or not.
    fn start(&mut self) -> bool;

    fn parse(&self, qemu: Qemu) -> result::Result<Self::Commands, CommandError>;
}

pub trait Command<CM, I, S>: Clone + Debug
where
    CM: CommandManager<I, S>,
{
    /// Used to know whether the command can be run during a backdoor, or if it is necessary to go out of
    /// the QEMU VM to run the command.
    // TODO: Use const when stabilized
    fn usable_at_runtime(&self) -> bool;

    /// Command handler.
    ///     - `ret_reg`: The register in which the guest return value should be written, if any.
    /// Returns
    ///     - `InnerHandlerResult`: How the high-level handler should behave
    fn run<EMU>(
        &self,
        emu: &mut EMU,
        ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>>
    where
        EMU: Emulator<I, S, CommandManager = CM>;
}

#[derive(Debug, Clone, Error)]
pub enum CommandError {
    #[error("unknown command: {0:?}")]
    UnknownCommand(GuestReg),
    #[error(transparent)]
    RWError(#[from] QemuRWError),
    #[error("version mismatch: received {0}, expected {1}")]
    VersionDifference(u64, u64),
    #[error("test mismatch: received {0:?}, expected {1:?}")]
    TestDifference(GuestReg, GuestReg),
    #[error("invalid parameters")]
    InvalidParameters,
    #[error("command manager started twice")]
    StartedTwice,
    #[error("end command received before start")]
    EndBeforeStart,
    #[error("wrong usage")]
    WrongUsage,
}

#[derive(Debug, Copy, Clone)]
pub struct NopCommandManager;
impl<I, S> CommandManager<I, S> for NopCommandManager {
    type Commands = NopCommand;
    type InputWriter = NopInputWriter;

    fn has_started(&self) -> bool {
        false
    }

    fn start(&mut self) -> bool {
        false
    }

    fn parse(&self, _qemu: Qemu) -> result::Result<Self::Commands, CommandError> {
        Ok(NopCommand)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NopCommand;

impl Display for NopCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "NopCommand")
    }
}

impl<CM, I, S> Command<CM, I, S> for NopCommand
where
    CM: CommandManager<I, S>,
{
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>>
    where
        EMU: Emulator<I, S, CommandManager = CM>,
    {
        Ok(None)
    }
}
