use crate::{EmulatorError, Result, emu::InputLocation, emu::InputWriter, qemu::Qemu};
use libaflmm::{inputs::Input, states::State};
use std::cell::OnceCell;
use std::cmp::min;
use std::ptr;
use std::slice;

#[derive(Clone, Debug)]
pub struct StdNyxInputWriter {
    input_location: OnceCell<InputLocation>,
    input_struct_location: OnceCell<InputLocation>,
    max_input_size: usize,
}

impl Default for StdNyxInputWriter {
    fn default() -> Self {
        Self {
            input_location: OnceCell::new(),
            input_struct_location: OnceCell::new(),
            max_input_size: 1024 * 1024,
        }
    }
}

pub trait NyxInputWriter<I, S>: InputWriter<I, S> {
    fn set_input_struct_location(&mut self, location: InputLocation) -> Result<()>;

    fn input_struct_location(&self) -> Option<&InputLocation>;

    fn max_input_size(&self) -> usize;
}

impl StdNyxInputWriter {
    pub fn max_input_size(&self) -> usize {
        self.max_input_size
    }
}

impl<I, S> InputWriter<I, S> for StdNyxInputWriter
where
    I: Input,
    S: State<Input = I>,
{
    fn write_input(&mut self, _qemu: Qemu, _state: &mut S, input: &I) -> Result<()> {
        let input_len =
            i32::try_from(min(self.max_input_size, input.target_bytes().len())).unwrap();

        let kafl_payload = libvharness_sys::kAFL_payload {
            size: input_len,
            ..Default::default()
        };

        let kafl_payload_buf = unsafe {
            slice::from_raw_parts(
                ptr::from_ref(&kafl_payload) as *const u8,
                size_of::<libvharness_sys::kAFL_payload>(),
            )
        };

        // TODO: manage endianness correctly.
        self.input_struct_location
            .get_mut()
            .unwrap()
            .write(kafl_payload_buf);

        // write struct first
        self.input_location
            .get_mut()
            .unwrap()
            .write(input.target_bytes().as_ref());

        Ok(())
    }

    fn set_input_location(&mut self, location: InputLocation) -> Result<()> {
        self.input_location
            .set(location)
            .or(Err(EmulatorError::MultipleInputLocationDefinition))
    }

    fn input_location(&self) -> Option<&InputLocation> {
        self.input_location.get()
    }

    fn input_size(&self, state: &mut S, input: &I) -> usize {
        if let Some(input_location) = self.input_location.get() {
            input_location.input_size(state.context_mut().len(input))
        } else {
            0
        }
    }
}

impl<I, S> NyxInputWriter<I, S> for StdNyxInputWriter
where
    I: Input,
    S: State<Input = I>,
{
    fn set_input_struct_location(&mut self, location: InputLocation) -> Result<()> {
        self.input_struct_location
            .set(location)
            .or(Err(EmulatorError::MultipleInputLocationDefinition))
    }

    fn input_struct_location(&self) -> Option<&InputLocation> {
        self.input_struct_location.get()
    }

    fn max_input_size(&self) -> usize {
        self.max_input_size
    }
}
