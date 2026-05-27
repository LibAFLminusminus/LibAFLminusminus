//! # Nyx API Command handlers
//!
//! Nyx command handlers.
//! Makes it possible to run Nyx targets in `LibAFL` QEMU.
//! The [Nyx API](https://github.com/IntelLabs/kafl.targets/blob/master/nyx_api.h) refers to the hypercalls used in Nyx to communicate with the fuzzer, not to the fuzzer itself.
//! This is mostly a convenient way to run Nyx-compatible targets in `LibAFL` QEMU directly, without having to change a single bit of the target files.

use super::{Command, CommandError};
use crate::{
    Result,
    arch::{GuestReg, Regs},
    command::CommandManager,
    emu::{Emulator, EmulatorError, EmulatorRunResult, InputLocation, SnapshotManager},
    modules::HasAddressFilterTuple,
};
use crate::{modules::HasPageFilterTuple, qemu::QemuMemoryChunk};
use libaflmm::executors::ExitKind;
use libaflmm_qemu_sys::{GuestAddr, GuestVirtAddr};
use paste::paste;
use std::{fmt::Debug, mem::offset_of, ops::Range, ptr, slice};

pub mod parser;
use parser::{
    AcquireCommandParser, GetHostConfigCommandParser, GetPayloadCommandParser,
    NextPayloadCommandParser, PanicCommandParser, PrintfCommandParser, RangeSubmitCommandParser,
    ReleaseCommandParser, SetAgentConfigCommandParser, SubmitCR3CommandParser,
    SubmitPanicCommandParser, UserAbortCommandParser,
};

macro_rules! define_nyx_command_manager {
    ($name:ident, [$($command:ty),+], [$($native_command_parser:ty),+]) => {
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
                    emu::{EmulatorRunResult, input_writer::StdNyxInputWriter},
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
                    type InputWriter = StdNyxInputWriter;

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
                        let nyx_backdoor = qemu.read_reg(Regs::Rax)? as c_uint;
                        let cmd_id = qemu.read_reg(Regs::Rbx)? as c_uint;

                        // Check nyx backdoor correctness
                        debug_assert_eq!(nyx_backdoor, libvharness_sys::HYPERCALL_KAFL_RAX_ID);

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

                impl<I, S> Command<I, S> for [<$name Commands>]
                where
                    I: Unpin,
                    S: Unpin,
                {
                    fn usable_at_runtime(&self) -> bool {
                        match self {
                            $([<$name Commands>]::$command(cmd) => <[<$command Command>] as Command<I, S>>::usable_at_runtime(cmd)),+
                        }
                    }

                    fn run<EMU>(&self,
                        emu: &mut EMU,
                        ret_reg: Option<Regs>
                    ) -> Result<Option<EmulatorRunResult>>
                    where
                        EMU: Emulator<I, S>
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

define_nyx_command_manager!(
    Nyx,
    [
        Acquire,
        Release,
        GetHostConfig,
        SetAgentConfig,
        Printf,
        GetPayload,
        NextPayload,
        SubmitCR3,
        Panic,
        SubmitPanic,
        UserAbort,
        RangeSubmit
    ],
    [
        AcquireCommandParser,
        ReleaseCommandParser,
        GetHostConfigCommandParser,
        SetAgentConfigCommandParser,
        PrintfCommandParser,
        GetPayloadCommandParser,
        NextPayloadCommandParser,
        SubmitCR3CommandParser,
        SubmitPanicCommandParser,
        PanicCommandParser,
        UserAbortCommandParser,
        RangeSubmitCommandParser
    ]
);

#[derive(Debug, Clone)]
pub struct AcquireCommand;
impl<I, S> Command<I, S> for AcquireCommand {
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct GetPayloadCommand {
    input_struct_location: GuestVirtAddr,
}

impl GetPayloadCommand {
    #[must_use]
    pub fn new(input_struct_location: GuestVirtAddr) -> Self {
        Self {
            input_struct_location,
        }
    }
}

impl<I, S> Command<I, S> for GetPayloadCommand
where
    I: Unpin,
    S: Unpin,
{
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        let qemu = emu.qemu();

        let struct_addr = self.input_struct_location;
        let input_addr = self.input_struct_location
            + offset_of!(libvharness_sys::kAFL_payload, data) as GuestVirtAddr;

        let payload_struct_mem_chunk = QemuMemoryChunk::virt(
            struct_addr,
            size_of::<libvharness_sys::kAFL_payload>() as GuestReg,
            qemu.current_cpu().unwrap(),
        );
        let payload_mem_chunk = QemuMemoryChunk::virt(
            input_addr,
            emu.driver().input_setter().max_input_size() as GuestReg,
            qemu.current_cpu().unwrap(),
        );

        // Save input struct location for next runs
        emu.input_writer_mut()
            .set_input_struct_location(InputLocation::new(qemu, &payload_struct_mem_chunk, None))
            .unwrap();

        // Save input location for next runs
        emu.input_writer_mut()
            .set_input_location(InputLocation::new(qemu, &payload_mem_chunk, None))
            .unwrap();

        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct NextPayloadCommand;

impl<I, S> Command<I, S> for NextPayloadCommand {
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        let qemu = emu.qemu();

        if !emu.command_manager_mut().start() {
            log::debug!("Creating snapshot.");

            // Snapshot VM
            let snapshot_id = emu.snapshot_manager_mut().save(qemu);

            // Set snapshot ID to restore to after fuzzing ends
            emu.driver_mut()
                .set_snapshot_id(snapshot_id)
                .map_err(|_| EmulatorError::MultipleSnapshotDefinition)?;

            // Auto page filtering if option is enabled
            #[cfg(feature = "systemmode")]
            if emu.driver_mut().allow_page_on_start() {
                if let Some(paging_id) = qemu.current_cpu().unwrap().current_paging_id() {
                    log::info!("Filter: allow page ID {paging_id}.");
                    emu.modules_mut().modules_mut().allow_page_id_all(paging_id);
                }
            }

            // Make sure JIT cache is empty just before starting
            qemu.flush_jit();

            log::info!("Fuzzing starts");

            return Ok(Some(EmulatorRunResult::FuzzingStarts));
        }

        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct SubmitCR3Command;

impl<I, S> Command<I, S> for SubmitCR3Command {
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        let qemu = emu.qemu();

        if let Some(current_cpu) = qemu.current_cpu() {
            if let Some(paging_id) = current_cpu.current_paging_id() {
                log::info!("Filter: allow page ID {paging_id}.");
                emu.modules_mut().modules_mut().allow_page_id_all(paging_id);
                Ok(None)
            } else {
                log::warn!("No paging id found for current cpu");
                Err(EmulatorError::CommandError(CommandError::WrongUsage))
            }
        } else {
            log::error!("No current cpu found");
            Err(EmulatorError::CommandError(CommandError::WrongUsage))
        }
    }
}

#[derive(Debug, Clone)]
pub struct RangeSubmitCommand {
    allowed_range: Range<GuestAddr>,
}

impl RangeSubmitCommand {
    pub fn new(allowed_range: Range<GuestAddr>) -> Self {
        Self { allowed_range }
    }
}

impl<I, S> Command<I, S> for RangeSubmitCommand {
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        log::info!("Allow address range: {:#x?}", self.allowed_range);

        const EMPTY_RANGE: Range<GuestAddr> = 0..0;

        if self.allowed_range == EMPTY_RANGE {
            log::warn!(
                "The given range is {:#x?}, which is most likely invalid. It is most likely a guest error.",
                EMPTY_RANGE
            );
            log::warn!(
                "Hint: make sure the range is not getting optimized out (the volatile keyword may help you)."
            );
        }

        emu.modules_mut()
            .modules_mut()
            .allow_address_range_all(&self.allowed_range);
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct PanicCommand;

impl<I, S> Command<I, S> for PanicCommand {
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        let qemu = emu.qemu();

        if !emu.command_manager_mut().has_started() {
            return Err(EmulatorError::CommandError(CommandError::EndBeforeStart));
        }

        let snapshot_id = emu
            .driver_mut()
            .snapshot_id()
            .ok_or(EmulatorError::SnapshotNotFound)?;

        log::debug!("Restoring snapshot");
        emu.snapshot_manager_mut().restore(qemu, &snapshot_id)?;

        emu.snapshot_manager_mut().check(qemu, &snapshot_id)?;

        Ok(Some(EmulatorRunResult::EndOfRun(ExitKind::Crash)))
    }
}

#[derive(Debug, Clone)]
pub struct SubmitPanicCommand;

impl<I, S> Command<I, S> for SubmitPanicCommand {
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        // TODO: add breakpoint to submit panic addr / page and associate it with a panic command
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
pub struct UserAbortCommand {
    content: String,
}

impl UserAbortCommand {
    pub fn new(content: String) -> Self {
        Self { content }
    }
}

impl<I, S> Command<I, S> for UserAbortCommand {
    fn usable_at_runtime(&self) -> bool {
        true
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        log::error!("Nyx Guest Abort: {}", self.content);

        Ok(Some(EmulatorRunResult::ShutdownRequest))
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseCommand;
impl<I, S> Command<I, S> for ReleaseCommand {
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        let qemu = emu.qemu();

        if emu.command_manager().has_started() {
            log::debug!("Release: end of fuzzing run. Restoring...");

            let snapshot_id = emu
                .driver_mut()
                .snapshot_id()
                .ok_or(EmulatorError::SnapshotNotFound)?;

            log::debug!("Restoring snapshot");
            emu.snapshot_manager_mut().restore(qemu, &snapshot_id)?;

            #[cfg(feature = "paranoid_debug")]
            emu.snapshot_manager_mut().check(qemu, &snapshot_id)?;

            Ok(Some(EmulatorRunResult::EndOfRun(ExitKind::Ok)))
        } else {
            log::debug!("Early release. Skipping...");

            Ok(None)
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetHostConfigCommand {
    host_config_location: QemuMemoryChunk,
}

impl GetHostConfigCommand {
    #[must_use]
    pub fn new(host_config_location: QemuMemoryChunk) -> Self {
        Self {
            host_config_location,
        }
    }
}

impl<I, S> Command<I, S> for GetHostConfigCommand {
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        // TODO: check this against fuzzer code
        let host_config = libvharness_sys::host_config_t {
            bitmap_size: 0,
            ijon_bitmap_size: 0,
            payload_buffer_size: 0,
            worker_id: 0,
            host_magic: libvharness_sys::NYX_HOST_MAGIC,
            host_version: libvharness_sys::NYX_HOST_VERSION,
        };

        let host_config_buf = unsafe {
            slice::from_raw_parts(
                ptr::from_ref(&host_config) as *const u8,
                size_of::<libvharness_sys::host_config_t>(),
            )
        };

        let qemu = emu.qemu();

        self.host_config_location
            .write(qemu, host_config_buf)
            .unwrap();

        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct PrintfCommand {
    content: String,
}

impl PrintfCommand {
    #[must_use]
    pub fn new(content: String) -> Self {
        Self { content }
    }
}

impl<I, S> Command<I, S> for PrintfCommand {
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        println!("hprintf: {}", self.content);
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct SetAgentConfigCommand {
    agent_config: libvharness_sys::agent_config_t,
}

impl SetAgentConfigCommand {
    #[must_use]
    pub fn new(agent_config: libvharness_sys::agent_config_t) -> Self {
        Self { agent_config }
    }
}

impl<I, S> Command<I, S> for SetAgentConfigCommand {
    fn usable_at_runtime(&self) -> bool {
        false
    }

    fn run<EMU: Emulator<I, S>>(
        &self,
        _emu: &mut EMU,
        _ret_reg: Option<Regs>,
    ) -> Result<Option<EmulatorRunResult>> {
        let agent_magic = self.agent_config.agent_magic;
        let agent_version = self.agent_config.agent_version;

        assert_eq!(agent_magic, libvharness_sys::NYX_AGENT_MAGIC);
        assert_eq!(agent_version, libvharness_sys::NYX_AGENT_VERSION);

        // TODO: use agent config

        Ok(None)
    }
}
