use libafl::{
    Error,
    executors::ExitKind,
    inputs::{BytesInput, HasTargetBytes},
};
use libafl_bolts::AsSlice;
use libafl_qemu::{ArchExtras, GuestAddr, GuestReg, MmapPerms, Qemu, Regs, elf::EasyElf};

pub struct Harness {
    qemu: Qemu,
    input_addr: GuestAddr,
    pc: GuestReg,
    stack_ptr: GuestReg,
    ret_addr: GuestAddr,
}

pub const MAX_INPUT_SIZE: usize = 1_048_576; // 1MB

impl Harness {
    /// Helper function to find the function we want to fuzz.
    fn start_pc(qemu: Qemu) -> Result<GuestAddr, Error> {
        let mut elf_buffer = Vec::new();
        let elf = EasyElf::from_file(qemu.binary_path(), &mut elf_buffer)?;

        let start_pc = elf
            .resolve_symbol("LLVMFuzzerTestOneInput", qemu.load_addr())
            .ok_or_else(|| Error::empty_optional("Symbol LLVMFuzzerTestOneInput not found"))?;
        Ok(start_pc)
    }

    /// Initialize the emulator, run to the entrypoint (or jump there) and return the [`Harness`] struct
    pub fn init(qemu: Qemu) -> Result<Harness, Error> {
        let start_pc = Self::start_pc(qemu)?;
        log::info!("start_pc @ {start_pc:#x}");

        qemu.entry_break(start_pc);

        let ret_addr: GuestAddr = qemu
            .read_return_address()
            .map_err(|e| Error::unknown(format!("Failed to read return address: {e:?}")))?;
        log::info!("ret_addr = {ret_addr:#x}");
        qemu.set_breakpoint(ret_addr);

        let input_addr = qemu
            .map_private(0, MAX_INPUT_SIZE, MmapPerms::ReadWrite)
            .map_err(|e| Error::unknown(format!("Failed to map input buffer: {e:}")))?;

        let pc: GuestReg = qemu
            .read_reg(Regs::Pc)
            .map_err(|e| Error::unknown(format!("Failed to read PC: {e:?}")))?;

        let stack_ptr = qemu
            .read_reg(Regs::Sp)
            .map_err(|e| Error::unknown(format!("Failed to read stack pointer: {e:?}")))?;

        let ret_addr: GuestAddr = qemu
            .read_return_address()
            .map_err(|e| Error::unknown(format!("Failed to read return address: {e:?}")))?;

        Ok(Harness {
            qemu,
            input_addr,
            pc,
            stack_ptr,
            ret_addr,
        })
    }
}
