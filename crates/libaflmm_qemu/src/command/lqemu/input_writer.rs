use crate::{
    Result,
    arch::GuestReg,
    emu::{EmulatorError, InputLocation, InputWriter},
    qemu::Qemu,
};
use libaflmm::{
    inputs::{Input, InputContext},
    states::State,
};
use libaflmm_bolts::AsSlice;
use std::cell::OnceCell;

#[derive(Debug, Default, Clone)]
pub struct LqemuInputWriter {
    input_location: OnceCell<InputLocation>,
}

impl<I, S> InputWriter<I, S> for LqemuInputWriter
where
    I: Input,
    S: State<Input = I>,
{
    fn write_input(&mut self, _qemu: Qemu, state: &mut S, input: &I) -> Result<()> {
        if let Some(input_location) = self.input_location.get_mut() {
            let ret_value = input_location.write(state.context_mut().to_bytes(input).as_slice());

            if let Some(reg) = input_location.ret_register() {
                input_location
                    .cpu()
                    .write_reg(*reg, ret_value as GuestReg)
                    .unwrap();
            }
        }

        Ok(())
    }

    fn set_input_location(&mut self, location: InputLocation) -> Result<()> {
        self.input_location
            .set(location)
            .or(Err(EmulatorError::MultipleInputLocationDefinition.into()))
    }

    fn input_location(&self) -> Option<&InputLocation> {
        self.input_location.get()
    }
}
