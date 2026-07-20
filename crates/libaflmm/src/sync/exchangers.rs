use std::marker::PhantomData;

use crate::Result;
use crate::inputs::Input;
use crate::sync::InputExchanger;

pub struct IdentityExchanger<'a> {
    phantom: PhantomData<&'a ()>,
}

impl<'a, I> InputExchanger<I> for IdentityExchanger<'a>
where
    &'a I: Input,
    I: 'a,
{
    type InputHandle = &'a I;

    fn create_handle(&mut self, input: &I) -> Result<Self::InputHandle> {
        Ok(input)
    }

    fn handle_to_input(&mut self, handle: Self::InputHandle) -> Result<I> {
        Ok(handle)
    }
}
