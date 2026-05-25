use libaflmm::{Result, prelude::*};
use libaflmm_qemu::prelude::*;
use std::{ops::Range, slice};

use crate::options::CommonOptions;

pub struct Harness {
    input_addr: GuestAddr,
    pc: GuestReg,
    stack_ptr: GuestReg,
    ret_addr: GuestAddr,
}

pub const MAX_INPUT_SIZE: usize = 1 << 20; // 1MB

impl Harness {
    /// Helper function to find the function we want to fuzz.
    fn start_pc(qemu: Qemu) -> Result<GuestAddr> {
        let mut elf_buffer = Vec::new();
        let elf = EasyElf::from_file(qemu.binary_path(), &mut elf_buffer)?;

        let start_pc = elf
            .resolve_symbol("LLVMFuzzerTestOneInput", qemu.load_addr())
            .ok_or_else(|| empty_optional!("Symbol LLVMFuzzerTestOneInput not found"))?;
        Ok(start_pc)
    }

    fn coverage_filter(
        emu: &mut impl Emulator,
        options: &CommonOptions,
    ) -> Result<StdAddressFilter> {
        /* Conversion is required on 32-bit targets, but not on 64-bit ones */
        if let Some(includes) = &options.include {
            #[cfg_attr(target_pointer_width = "64", allow(clippy::useless_conversion))]
            let rules = includes
                .iter()
                .map(|x| Range {
                    start: x.start.into(),
                    end: x.end.into(),
                })
                .collect::<Vec<Range<GuestAddr>>>();
            Ok(StdAddressFilter::allow_list(rules))
        } else if let Some(excludes) = &options.exclude {
            #[cfg_attr(target_pointer_width = "64", allow(clippy::useless_conversion))]
            let rules = excludes
                .iter()
                .map(|x| Range {
                    start: x.start.into(),
                    end: x.end.into(),
                })
                .collect::<Vec<Range<GuestAddr>>>();
            Ok(StdAddressFilter::deny_list(rules))
        } else {
            let mut elf_buffer = Vec::new();
            let elf = EasyElf::from_file(emu.binary_path(), &mut elf_buffer)?;
            let range = elf
                .get_section(".text", emu.load_addr())
                .ok_or_else(|| key_not_found!("Failed to find .text section"))?;
            Ok(StdAddressFilter::allow_list(vec![range]))
        }
    }

    /// Initialize the emulator, run to the entrypoint (or jump there) and return the [`Harness`] struct
    pub fn init<E>(emu: &mut E, options: &CommonOptions) -> Result<Harness>
    where
        E: Emulator,
        E::Command: From<StdCommands>,
    {
        let start_pc = Self::start_pc(emu.qemu())?;
        log::info!("start_pc @ {start_pc:#x}");

        // emu.entry_break(start_pc)?;

        let ret_addr: GuestAddr = emu
            .read_return_address()
            .map_err(|e| unknown!("Failed to read return address: {e}"))?;
        log::info!("ret_addr = {ret_addr:#x}");

        let input_addr = emu
            .map_private(0, MAX_INPUT_SIZE, MmapPerms::ReadWrite)
            .map_err(|e| unknown!("Failed to map input buffer: {e}"))?;

        let input_slice: *mut [u8] =
            unsafe { slice::from_raw_parts_mut(input_addr as *mut u8, MAX_INPUT_SIZE) };
        let input_box = unsafe { Box::from_raw(input_slice) };

        let cpu = emu.cpu_from_index(0).unwrap();

        emu.add_breakpoint(
            Breakpoint::with_command(
                start_pc,
                StdCommands::Start(StartCommand::new(InputLocation::new(input_box, None, cpu)))
                    .into(),
                false,
            ),
            true,
        );

        emu.add_breakpoint(
            Breakpoint::with_command(
                ret_addr,
                StdCommands::End(EndCommand::new(Some(ExitKind::Ok))).into(),
                false,
            ),
            true,
        );

        let pc: GuestReg = emu
            .read_reg(Regs::Pc)
            .map_err(|e| unknown!("Failed to read PC: {e}"))?;

        let stack_ptr = emu
            .read_reg(Regs::Sp)
            .map_err(|e| unknown!("Failed to read stack pointer: {e}"))?;

        let ret_addr: GuestAddr = emu
            .read_return_address()
            .map_err(|e| unknown!("Failed to read return address: {e}"))?;

        Self::coverage_filter(emu, options)?;

        emu.start();

        Ok(Harness {
            input_addr,
            pc,
            stack_ptr,
            ret_addr,
        })
    }

    pub fn run<I, S: State<Input = I>>(
        &self,
        state: &mut S,
        input: &I,
        emu: &mut impl Emulator<Input = I, State = S>,
    ) -> Result<()> {
        let bytes = state.context_mut().to_bytes(input);
        let mut buf = bytes.iter().as_slice();
        let mut len = buf.len();
        if len > MAX_INPUT_SIZE {
            buf = &buf[0..MAX_INPUT_SIZE];
            len = MAX_INPUT_SIZE;
        }
        let len = len as GuestReg;

        emu.write_mem(self.input_addr, buf)
            .map_err(|e| runtime!("Failed to write to memory@{:#x}: {e:?}", self.input_addr))?;

        emu.write_reg(Regs::Pc, self.pc)
            .map_err(|e| runtime!("Failed to write PC: {e:?}"))?;

        emu.write_reg(Regs::Sp, self.stack_ptr)
            .map_err(|e| runtime!("Failed to write SP: {e:?}"))?;

        emu.write_return_address(self.ret_addr)
            .map_err(|e| runtime!("Failed to write return address: {e:?}"))?;

        emu.write_function_argument(0, self.input_addr as GuestReg)
            .map_err(|e| runtime!("Failed to write argument 0: {e:?}"))?;

        emu.write_function_argument(1, len)
            .map_err(|e| runtime!("Failed to write argument 1: {e:?}"))?;

        Ok(())
    }
}
