use crate::{arch::Regs, qemu::CPU};

/// The fuzzing input location.
///
/// We store the memory location to which the input should be written,
/// and the return register containing the number bytes effectively written.
#[derive(Debug, Clone)]
pub struct InputLocation {
    location: Box<[u8]>,
    ret_register: Option<Regs>,
    cpu: CPU,
}

impl InputLocation {
    #[must_use]
    pub fn new(location: Box<[u8]>, ret_register: Option<Regs>, cpu: CPU) -> Self {
        Self {
            location,
            ret_register,
            cpu,
        }
    }

    pub fn write(&mut self, input: &[u8]) -> usize {
        if input.len() < self.location.len() {
            self.location[..input.len()].copy_from_slice(input);
            input.len()
        } else if input.len() > self.location.len() {
            self.location.copy_from_slice(&input[..self.location.len()]);
            self.location.len()
        } else {
            self.location.copy_from_slice(input);
            input.len()
        }
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
