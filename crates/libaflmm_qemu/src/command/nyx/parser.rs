use super::{
    AcquireCommand, GetHostConfigCommand, GetPayloadCommand, NextPayloadCommand, PanicCommand,
    PrintfCommand, RangeSubmitCommand, ReleaseCommand, SetAgentConfigCommand, SubmitCR3Command,
    SubmitPanicCommand, UserAbortCommand,
};
use crate::{
    arch::{GuestReg, Regs},
    command::{CommandError, NativeCommandParser},
    qemu::{Qemu, QemuMemoryChunk},
    sync_exit::ExitArgs,
};
use enum_map::EnumMap;
use libaflmm_qemu_sys::{GuestAddr, GuestVirtAddr};
use libc::c_uint;
use std::ffi::CStr;

fn get_guest_string(qemu: Qemu, string_ptr_reg: Regs) -> Result<String, CommandError> {
    let str_addr = qemu.read_reg(string_ptr_reg)? as GuestVirtAddr;

    let mut msg_chunk: [u8; libvharness_sys::HPRINTF_MAX_SIZE as usize] =
        [0; libvharness_sys::HPRINTF_MAX_SIZE as usize];
    qemu.read_mem(str_addr.try_into().unwrap(), &mut msg_chunk)?;

    Ok(CStr::from_bytes_until_nul(&msg_chunk)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string())
}

pub struct AcquireCommandParser;
impl NativeCommandParser for AcquireCommandParser {
    type OutputCommand = AcquireCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_ACQUIRE;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        Ok(AcquireCommand)
    }
}

pub struct GetPayloadCommandParser;
impl NativeCommandParser for GetPayloadCommandParser {
    type OutputCommand = GetPayloadCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_GET_PAYLOAD;

    fn parse(
        qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        let payload_addr = qemu.read_reg(Regs::Rcx).unwrap() as GuestVirtAddr;

        Ok(GetPayloadCommand::new(payload_addr))
    }
}

pub struct SubmitCR3CommandParser;
impl NativeCommandParser for SubmitCR3CommandParser {
    type OutputCommand = SubmitCR3Command;
    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_SUBMIT_CR3;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        Ok(SubmitCR3Command)
    }
}

pub struct RangeSubmitCommandParser;
impl NativeCommandParser for RangeSubmitCommandParser {
    type OutputCommand = RangeSubmitCommand;
    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_RANGE_SUBMIT;

    fn parse(
        qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        let allowed_range_addr = qemu.read_reg(Regs::Rcx)? as GuestAddr;

        // # Safety
        // Range submit is represented with an array of 3 u64 in the Nyx API.
        let allowed_range: [u64; 3] = unsafe { qemu.read_mem_val(allowed_range_addr)? };

        Ok(RangeSubmitCommand::new(
            allowed_range[0] as GuestAddr..allowed_range[1] as GuestAddr,
        ))
    }
}

pub struct SubmitPanicCommandParser;
impl NativeCommandParser for SubmitPanicCommandParser {
    type OutputCommand = SubmitPanicCommand;
    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_SUBMIT_PANIC;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        Ok(SubmitPanicCommand)
    }
}

pub struct PanicCommandParser;
impl NativeCommandParser for PanicCommandParser {
    type OutputCommand = PanicCommand;
    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_PANIC;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        Ok(PanicCommand)
    }
}

pub struct UserAbortCommandParser;
impl NativeCommandParser for UserAbortCommandParser {
    type OutputCommand = UserAbortCommand;
    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_USER_ABORT;

    fn parse(
        qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        let msg = get_guest_string(qemu, Regs::Rcx)?;

        Ok(UserAbortCommand::new(msg))
    }
}

pub struct NextPayloadCommandParser;
impl NativeCommandParser for NextPayloadCommandParser {
    type OutputCommand = NextPayloadCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_NEXT_PAYLOAD;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        Ok(NextPayloadCommand)
    }
}

pub struct ReleaseCommandParser;
impl NativeCommandParser for ReleaseCommandParser {
    type OutputCommand = ReleaseCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_RELEASE;

    fn parse(
        _qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        Ok(ReleaseCommand)
    }
}

pub struct GetHostConfigCommandParser;
impl NativeCommandParser for GetHostConfigCommandParser {
    type OutputCommand = GetHostConfigCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_GET_HOST_CONFIG;

    fn parse(
        qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        let host_config_addr = qemu.read_reg(Regs::Rcx)? as GuestVirtAddr;

        Ok(GetHostConfigCommand::new(QemuMemoryChunk::virt(
            host_config_addr,
            GuestReg::try_from(size_of::<libvharness_sys::host_config_t>()).unwrap(),
            qemu.current_cpu().unwrap(),
        )))
    }
}

pub struct SetAgentConfigCommandParser;
impl NativeCommandParser for SetAgentConfigCommandParser {
    type OutputCommand = SetAgentConfigCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_SET_AGENT_CONFIG;

    fn parse(
        qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        let agent_config_addr = qemu.read_reg(Regs::Rcx)? as GuestAddr;

        // # Safety
        // We use the C struct directly to get the agent config
        let agent_config: libvharness_sys::agent_config_t =
            unsafe { qemu.read_mem_val(agent_config_addr)? };

        Ok(SetAgentConfigCommand::new(agent_config))
    }
}

pub struct PrintfCommandParser;
impl NativeCommandParser for PrintfCommandParser {
    type OutputCommand = PrintfCommand;

    const COMMAND_ID: c_uint = libvharness_sys::HYPERCALL_KAFL_PRINTF;

    fn parse(
        qemu: Qemu,
        _arch_regs_map: &'static EnumMap<ExitArgs, Regs>,
    ) -> Result<Self::OutputCommand, CommandError> {
        let msg = get_guest_string(qemu, Regs::Rcx)?;

        Ok(PrintfCommand::new(msg))
    }
}
