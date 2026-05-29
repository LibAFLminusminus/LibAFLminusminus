use crate::{arch::Regs, qemu::CPU};
use std::cmp::min;
use std::fmt::{Debug, Formatter};
use std::ptr::NonNull;
use std::slice;

/// The fuzzing input location.
///
/// We store the memory location to which the input should be written,
/// and the return register containing the number bytes effectively written.
#[derive(Clone)]
pub struct InputLocation {
    addr: NonNull<u8>,
    size: usize,
    ret_register: Option<Regs>,
    cpu: CPU,
}

impl Debug for InputLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let dst = unsafe { slice::from_raw_parts(self.addr.as_ptr(), self.size) };

        write!(
            f,
            "InputLocation @host addr {:#x} {{\n\tsize: {:#x} bytes\n\tcontent: {:x?}\n\tret_register: {:?}\n\tcpu: {:?}\n}}",
            self.addr.as_ptr() as usize,
            self.size,
            &dst[..min(dst.len(), 64)],
            self.ret_register,
            self.cpu,
        )
    }
}

impl InputLocation {
    #[must_use]
    pub fn new(location: &mut [u8], ret_register: Option<Regs>, cpu: CPU) -> Self {
        Self {
            addr: NonNull::new(location.as_mut_ptr()).unwrap(),
            size: location.len(),
            ret_register,
            cpu,
        }
    }

    #[must_use]
    pub fn input_size(&self, input_len: usize) -> usize {
        if input_len <= self.size {
            input_len
        } else {
            self.size
        }
    }

    pub fn write(&mut self, input: &[u8]) -> usize {
        let size = self.input_size(input.len());
        let dst = unsafe { slice::from_raw_parts_mut(self.addr.as_mut(), size) };

        dst.copy_from_slice(&input[..size]);

        size
    }

    #[must_use]
    pub fn ret_register(&self) -> &Option<Regs> {
        &self.ret_register
    }

    #[must_use]
    pub fn cpu(&self) -> CPU {
        self.cpu
    }
}
