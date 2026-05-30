use crate::Result;

pub mod aflpp;

pub mod nop;
pub use nop::{NopInputRepr, NopSynchronizer};

pub type StdSynchronizer = NopSynchronizer;

pub trait InputRepr<I> {
    fn load_input(&self) -> Result<I>;
}

pub trait Synchronizer<D, I> {
    type InputRepr: InputRepr<I>;

    /// Called when the worker with descriptor `desc` which will hold this synchronizer is created.
    fn on_create(&mut self) -> Result<()>;

    /// Called when another worker with descriptor `desc` has been created to signal it exists.
    /// This is guaranteed to be called after the other worker has been called himself with `on_create`.
    fn on_new_worker(&mut self, desc: &D) -> Result<()>;

    /// Report an input has been received
    fn report_input(&mut self, desc: &mut D, input_repr: Self::InputRepr) -> Result<()>;

    /// Ask for an input to synchronize
    fn sync_input(&mut self, desc: &mut D) -> Result<impl Iterator<Item = Self::InputRepr>>;
}
