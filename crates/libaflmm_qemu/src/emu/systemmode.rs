use crate::{
    arch::Regs,
    qemu::{CPU, HostMemorySegments, Qemu, QemuMemoryChunk},
};

/// The fuzzing input location.
///
/// We store the memory location to which the input should be written,
/// and the return register containing the number bytes effectively written.
#[derive(Debug, Clone)]
pub struct InputLocation {
    location: HostMemorySegments,
    ret_register: Option<Regs>,
    cpu: CPU,
}

impl InputLocation {
    #[must_use]
    pub fn new(qemu: Qemu, mem_chunk: &QemuMemoryChunk, ret_register: Option<Regs>) -> Self {
        let location = mem_chunk.to_host_segments(qemu);

        Self {
            location,
            ret_register,
            cpu: mem_chunk.cpu(),
        }
    }

    pub fn input_size(&self, input_len: usize) -> usize {
        let mut size = 0;
        for segment in self.location.segments() {
            size += segment.size()
        }

        if input_len <= size { input_len } else { size }
    }

    #[must_use]
    pub fn location(&self) -> &HostMemorySegments {
        &self.location
    }

    #[must_use]
    pub fn ret_register(&self) -> &Option<Regs> {
        &self.ret_register
    }

    #[must_use]
    pub fn cpu(&self) -> CPU {
        self.cpu
    }

    pub fn write(&mut self, input: &[u8]) -> usize {
        unsafe { self.location.write(input) }
    }
}
