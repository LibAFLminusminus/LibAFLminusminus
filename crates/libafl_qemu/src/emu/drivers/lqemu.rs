use std::cell::OnceCell;

use libafl::{
    inputs::{Input, InputContext},
    states::HasContext,
};
use libafl_bolts::AsSlice;

#[cfg(not(feature = "systemmode"))]
use crate::InputLocation;
#[cfg(feature = "systemmode")]
use crate::emu::systemmode::SystemInputLocation as InputLocation;
use crate::{EmulatorDriverError, GuestReg, InputSetter, Qemu};

#[derive(Debug, Default, Clone)]
pub struct LqemuInputSetter {
    input_location: OnceCell<InputLocation>,
}

impl<I, S> InputSetter<I, S> for LqemuInputSetter
where
    I: Input,
    S: HasContext<I>,
{
    fn write_input(
        &mut self,
        _qemu: Qemu,
        state: &mut S,
        input: &I,
    ) -> Result<(), EmulatorDriverError> {
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

    fn set_input_location(&mut self, location: InputLocation) -> Result<(), EmulatorDriverError> {
        self.input_location
            .set(location)
            .or(Err(EmulatorDriverError::MultipleInputLocationDefinition))
    }

    fn input_location(&self) -> Option<&InputLocation> {
        self.input_location.get()
    }
}
