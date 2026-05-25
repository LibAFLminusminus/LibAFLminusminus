#[cfg(feature = "usermode")]
use std::slice;
use std::{ffi::CStr, result, sync::OnceLock};

use enum_map::{EnumMap, enum_map};
use libaflmm::executors::ExitKind;
#[cfg(feature = "systemmode")]
use libaflmm_qemu_sys::GuestPhysAddr;
use libaflmm_qemu_sys::{GuestAddr, GuestVirtAddr};
use libc::c_uint;

use super::{
    AddressAllowCommand, EndCommand, LoadCommand, LqprintfCommand, NativeExitKind, SaveCommand,
    StartCommand, TestCommand, VersionCommand,
};
use crate::{
    arch::{GuestReg, Regs},
    command::{CommandError, NativeCommandParser},
    emu::InputLocation,
    qemu::{Qemu, QemuMemoryChunk},
    sync_exit::ExitArgs,
};
#[cfg(feature = "systemmode")]
use crate::{command::lqemu::SetMapCommand, emu::MapKind};

pub static EMU_EXIT_KIND_MAP: OnceLock<EnumMap<NativeExitKind, Option<ExitKind>>> = OnceLock::new();

#[cfg(feature = "systemmode")]
pub struct StartPhysCommandParser;

#[cfg(feature = "systemmode")]
impl NativeCommandParser for StartPhysCommandParser {
    type OutputCommand = StartCommand;

    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_START_PHYS.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let input_phys_addr: GuestPhysAddr = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])?.into();
        let max_input_size: GuestReg = qemu.read_reg(arch_regs_map[ExitArgs::Arg2])?;

        let memory_chunk =
            QemuMemoryChunk::phys(input_phys_addr, max_input_size, qemu.current_cpu().unwrap());

        Ok(StartCommand::new(InputLocation::new(
            qemu,
            &memory_chunk,
            Some(arch_regs_map[ExitArgs::Ret]),
        )))
    }
}

pub struct StartVirtCommandParser;

impl NativeCommandParser for StartVirtCommandParser {
    type OutputCommand = StartCommand;

    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_START_VIRT.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let input_virt_addr: GuestVirtAddr =
            qemu.read_reg(arch_regs_map[ExitArgs::Arg1])? as GuestVirtAddr;
        let max_input_size: GuestReg = qemu.read_reg(arch_regs_map[ExitArgs::Arg2])?;

        #[cfg(feature = "usermode")]
        {
            let memory_chunk = unsafe {
                slice::from_raw_parts_mut(input_virt_addr as *mut u8, max_input_size as usize)
            };

            Ok(StartCommand::new(InputLocation::new(
                memory_chunk,
                Some(arch_regs_map[ExitArgs::Ret]),
                qemu.current_cpu().unwrap(),
            )))
        }

        #[cfg(feature = "systemmode")]
        {
            let memory_chunk =
                QemuMemoryChunk::virt(input_virt_addr, max_input_size, qemu.current_cpu().unwrap());

            Ok(StartCommand::new(InputLocation::new(
                qemu,
                &memory_chunk,
                Some(arch_regs_map[ExitArgs::Ret]),
            )))
        }
    }
}

pub struct SaveCommandParser;

impl NativeCommandParser for SaveCommandParser {
    type OutputCommand = SaveCommand;

    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_SAVE.0;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        Ok(SaveCommand)
    }
}

pub struct LoadCommandParser;
impl NativeCommandParser for LoadCommandParser {
    type OutputCommand = LoadCommand;

    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_LOAD.0;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        Ok(LoadCommand)
    }
}

pub struct EndCommandParser;

impl NativeCommandParser for EndCommandParser {
    type OutputCommand = EndCommand;

    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_END.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let native_exit_kind: GuestReg = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])?;
        let native_exit_kind: result::Result<NativeExitKind, _> =
            u64::from(native_exit_kind).try_into();

        let exit_kind = native_exit_kind.ok().and_then(|k| {
            EMU_EXIT_KIND_MAP.get_or_init(|| {
                enum_map! {
                    NativeExitKind::Unknown => None,
                    NativeExitKind::Ok      => Some(ExitKind::Ok),
                    NativeExitKind::Crash   => Some(ExitKind::Crash)
                }
            })[k]
        });

        Ok(EndCommand::new(exit_kind))
    }
}

pub struct VersionCommandParser;
impl NativeCommandParser for VersionCommandParser {
    type OutputCommand = VersionCommand;

    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_VERSION.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let major = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])?.into();
        let minor = qemu.read_reg(arch_regs_map[ExitArgs::Arg2])?.into();

        Ok(VersionCommand::new(major, minor))
    }
}

pub struct VaddrFilterAllowRangeCommandParser;

impl NativeCommandParser for VaddrFilterAllowRangeCommandParser {
    type OutputCommand = AddressAllowCommand;

    const COMMAND_ID: c_uint =
        libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_VADDR_FILTER_ALLOW.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let vaddr_start: GuestAddr = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])? as GuestAddr;
        let vaddr_end: GuestAddr = qemu.read_reg(arch_regs_map[ExitArgs::Arg2])? as GuestAddr;

        Ok(AddressAllowCommand::new(vaddr_start..vaddr_end))
    }
}

pub struct LqprintfCommandParser;

impl NativeCommandParser for LqprintfCommandParser {
    type OutputCommand = LqprintfCommand;
    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_LQPRINTF.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let buf_addr: GuestAddr = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])? as GuestAddr;
        let str_size: usize = qemu
            .read_reg(arch_regs_map[ExitArgs::Arg2])?
            .try_into()
            .unwrap(); // without null byte
        let cpu = qemu.current_cpu().unwrap();

        let total_size = str_size + 1;

        let mem_chunk =
            QemuMemoryChunk::virt(buf_addr as GuestVirtAddr, total_size as GuestReg, cpu);

        let str_copy: Vec<u8> = mem_chunk.read_vec(qemu)?;

        let c_str: &CStr = CStr::from_bytes_with_nul(str_copy.as_slice()).unwrap();

        Ok(LqprintfCommand::new(c_str.to_str().unwrap().to_string()))
    }
}

pub struct TestCommandParser;

impl NativeCommandParser for TestCommandParser {
    type OutputCommand = TestCommand;
    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_TEST.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let received_value: GuestReg = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])?;

        Ok(TestCommand::new(
            received_value,
            GuestReg::from(libvharness_sys::LIBAFL_QEMU_TEST_VALUE),
        ))
    }
}

#[cfg(feature = "systemmode")]
pub struct SetMapCommandParser;

#[cfg(feature = "systemmode")]
impl NativeCommandParser for SetMapCommandParser {
    type OutputCommand = SetMapCommand;
    const COMMAND_ID: c_uint = libvharness_sys::LibaflQemuCommand_LIBAFL_QEMU_COMMAND_SET_MAP.0;

    fn parse(
        qemu: Qemu,
        arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> result::Result<Self::OutputCommand, CommandError> {
        let map_addr: GuestAddr = qemu.read_reg(arch_regs_map[ExitArgs::Arg1])? as GuestAddr;
        let map: libvharness_sys::lqemu_map = unsafe { qemu.read_mem_val(map_addr)? };

        let kind = match map.map_kind {
            libvharness_sys::lqemu_map_kind_LQEMU_MAP_COV => MapKind::Cov,

            libvharness_sys::lqemu_map_kind_LQEMU_MAP_CMP => MapKind::Cmp,

            _ => return Err(CommandError::InvalidParameters),
        };

        let map = match map.addr_kind {
            libvharness_sys::lqemu_addr_kind_LQEMU_ADDR_PHYS => QemuMemoryChunk::phys(
                GuestPhysAddr::from(map.addr),
                map.len as GuestReg,
                qemu.current_cpu().unwrap(),
            ),

            libvharness_sys::lqemu_addr_kind_LQEMU_ADDR_VIRT => QemuMemoryChunk::virt(
                map.addr as GuestVirtAddr,
                map.len as GuestReg,
                qemu.current_cpu().unwrap(),
            ),

            _ => return Err(CommandError::InvalidParameters),
        };

        Ok(SetMapCommand::new(kind, map))
    }
}
