#![cfg(cpu_target = "x86_64")]
//! Test setting PC at the middle of a translation block.
//! The idea is to be able to change PC even while the guest is running.
//! With the old method, we were simply writing to the guest PC register, which does not
//! correctly set the actual execution to the given PC.

use crate::common::{boot_qemu, find_symbol};
use libaflmm_qemu::{GuestAddr, arch::Regs, qemu::Qemu};

#[path = "../common.rs"]
mod common;

unsafe extern "C" fn new_pc_hook(target_new_pc: u64, _pc: GuestAddr) {
    let qemu = Qemu::get().unwrap();

    let pc = qemu.read_reg(Regs::Pc).unwrap();

    println!("PC = {pc:#x}");
    println!("new PC = {target_new_pc:#x}");

    qemu.write_reg(Regs::Pc, target_new_pc).unwrap();
}

#[test]
fn set_pc() {
    let qemu = boot_qemu();

    let target_addr = find_symbol(qemu, "target");
    let target_new_pc_addr = find_symbol(qemu, "target_new_pc");

    // print "hello" each time the instruction at `target` is executed
    qemu.hooks()
        .add_instruction_hooks(target_new_pc_addr as u64, target_addr, new_pc_hook, true);

    unsafe {
        qemu.run().unwrap();
    }
}
