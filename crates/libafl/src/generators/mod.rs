//! Generators may generate bytes or, in general, data, for inputs.

use alloc::vec::Vec;
use core::{cmp::max, marker::PhantomData, num::NonZeroUsize};

use libafl_bolts::rands::Rand;

use crate::{Error, inputs::bytes::BytesInput, non_zero};

#[cfg(feature = "nautilus")]
pub mod nautilus;
#[cfg(feature = "nautilus")]
pub use nautilus::*;

/// Generators can generate inputs.
pub trait Generator<I, R, S> {
    /// Generate a new input
    fn generate(&mut self, _rand: &mut R, state: &S) -> Result<I, Error>;
}

/// Iterators may be used as generators.
///
/// `generate` throws a [`Error::Empty`] if an input is requested but
/// [`Iterator::next`] returns `None`.
impl<T, I, S, R: Rand> Generator<I, R, S> for T
where
    T: Iterator<Item = I>,
{
    fn generate(&mut self, _rand: &mut R, _state: &S) -> Result<I, Error> {
        match self.next() {
            Some(i) => Ok(i),
            None => Err(Error::empty(
                "No more items in iterator when generating inputs",
            )),
        }
    }
}


#[derive(Debug, Clone)]
/// Generates random bytes
pub struct RandBytesGenerator {
    min_size: NonZeroUsize,
    max_size: NonZeroUsize,
}

impl<R, S> Generator<BytesInput, R, S> for RandBytesGenerator 
where 
    R: Rand,
{
    fn generate(&mut self, rand: &mut R, _state: &S) -> Result<BytesInput, Error> {
        let mut size = rand.between(self.min_size.get(), self.max_size.get());
        size = max(size, 1);
        let random_bytes: Vec<u8> = (0..size)
            .map(|_| rand.below(non_zero!(256)) as u8)
            .collect();
        Ok(BytesInput::new(random_bytes))
    }
}

impl RandBytesGenerator {
    /// Returns a new [`RandBytesGenerator`], generating up to `max_size` random bytes.
    #[must_use]
    pub fn new(max_size: NonZeroUsize) -> Self {
        Self {
            min_size: non_zero!(1),
            max_size,
        }
    }

    /// Returns a new [`RandBytesGenerator`], generating from `min_size` up to `max_size` random bytes.
    #[must_use]
    pub fn with_min_size(min_size: NonZeroUsize, max_size: NonZeroUsize) -> Self {
        Self { min_size, max_size }
    }
}

#[derive(Debug, Clone)]
/// Generates random printable characters
pub struct RandPrintablesGenerator {
    min_size: NonZeroUsize,
    max_size: NonZeroUsize,
}

impl<R: Rand, S> Generator<BytesInput, R, S> for RandPrintablesGenerator {
    fn generate(&mut self, rand: &mut R, _state: &S) -> Result<BytesInput, Error> {
        let mut size = rand.between(self.min_size.get(), self.max_size.get());
        size = max(size, 1);
        let printables = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz \t\n!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".as_bytes();
        let random_bytes: Vec<u8> = (0..size)
            .map(|_| *rand.choose(printables).unwrap())
            .collect();
        Ok(BytesInput::new(random_bytes))
    }
}

impl RandPrintablesGenerator {
    /// Returns a new [`RandBytesGenerator`], generating up to `max_size` random bytes.
    #[must_use]
    pub fn new(max_size: NonZeroUsize) -> Self {
        Self {
            min_size: non_zero!(1),
            max_size,
        }
    }

    /// Returns a new [`RandPrintablesGenerator`], generating from `min_size` up to `max_size` random bytes.
    #[must_use]
    pub fn with_min_size(min_size: NonZeroUsize, max_size: NonZeroUsize) -> Self {
        Self { min_size, max_size }
    }
}
