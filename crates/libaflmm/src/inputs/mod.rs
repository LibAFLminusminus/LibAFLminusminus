//! Inputs are the actual contents sent to a target for each exeuction.

use alloc::{
    string::String,
    vec::{Drain, Splice, Vec},
};
use core::{
    clone::Clone,
    fmt::Debug,
    hash::Hash,
    ops::{DerefMut, RangeBounds},
};
use std::{fs::File, io::Read, path::Path};

use libaflmm_bolts::fs::write_file_atomic;
use libaflmm_bolts::{
    Error, HasLen, generic_hash_std,
    ownedref::{OwnedMutSlice, OwnedSlice},
    subrange::{SubRangeMutSlice, SubRangeSlice},
};
use serde::{Deserialize, Serialize};

pub mod bytes;
pub use bytes::{BytesContext, BytesInput};

pub mod value;
pub use value::ValueInput;

pub mod bytessub;
pub use bytessub::BytesSubInput;

#[cfg(feature = "nautilus")]
pub mod nautilus;
#[cfg(feature = "nautilus")]
pub use nautilus::NautilusInput;

pub type StdInput = BytesInput;
pub type StdContext = BytesContext;

/// A wrapper type that allows us to use mutators for Mutators for `&mut `[`Vec`].
#[deprecated(since = "0.15.0", note = "Use &mut Vec<u8> directly")]
pub type MutVecInput<'a> = &'a mut Vec<u8>;

/// An input for the target
pub trait Input: Clone + Serialize + serde::de::DeserializeOwned + Debug + Hash {
    /// Write this input to the file
    fn to_file(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_file_atomic(path, &postcard::to_allocvec(self)?)
    }

    /// Load the content of this input from a file
    fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut file = File::open(path)?;
        let mut bytes = vec![];
        file.read_to_end(&mut bytes)?;
        Ok(postcard::from_bytes(&bytes)?)
    }

    /// Generate a name for this input, the user is responsible for making each name of testcase unique.
    fn generate_name(&self) -> String {
        format!("{:016x}", generic_hash_std(self))
    }
}

/// Contains mutable bytes
pub trait HasMutatorBytes: HasLen {
    /// The bytes
    fn mutator_bytes(&self) -> &[u8];

    /// The bytes to mutate
    fn mutator_bytes_mut(&mut self) -> &mut [u8];

    /// Creates a [`SubRangeSlice`] from this input, that can be used to slice a byte array.
    fn sub_bytes<R>(&self, range: R) -> SubRangeSlice<'_, u8>
    where
        R: RangeBounds<usize>,
    {
        SubRangeSlice::new(OwnedSlice::from(self.mutator_bytes()), range)
    }

    /// Creates a [`SubRangeMutSlice`] from this input, that can be used to slice a byte array.
    fn sub_bytes_mut<R>(&mut self, range: R) -> SubRangeMutSlice<'_, u8>
    where
        R: RangeBounds<usize>,
    {
        SubRangeMutSlice::new(OwnedMutSlice::from(self.mutator_bytes_mut()), range)
    }

    /// Creates a [`BytesSubInput`] from this input, that can be used for local mutations.
    fn sub_input<R>(&mut self, range: R) -> BytesSubInput<'_, Self>
    where
        R: RangeBounds<usize>,
    {
        BytesSubInput::new(self, range)
    }
}

/// Contains resizable bytes
pub trait ResizableMutator<T> {
    /// Resize the mutator content to a given new size.
    /// Use `value` to fill new slots in case the buffer grows.
    /// See [`Vec::splice`].
    fn resize(&mut self, new_len: usize, value: T);

    /// Extends the given buffer with an iterator. See [`alloc::vec::Vec::extend`]
    fn extend<'a, I: IntoIterator<Item = &'a T>>(&mut self, iter: I)
    where
        T: 'a;

    /// Splices the given target values according to [`Vec::splice`]'s rules
    fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>;

    /// Drains the given target value according to [`Vec::drain`]'s rules
    fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>;
}

/// [`InputContext`] helps the conversion of [`Input`] type to byte slice.
pub trait InputContext {
    type Input;

    /// Turns this `input` to slice
    fn to_bytes<'a>(&mut self, input: &'a Self::Input) -> OwnedSlice<'a, u8>;

    /// Get the `input` size in bytes
    fn len<'a>(&self, input: &'a Self::Input) -> usize;
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
/// [`NopContext`] just returns an empty [`OwnedSlice`]
pub struct NopContext;

/// An input for tests, mainly. There is no real use much else.
#[derive(Clone, Serialize, Deserialize, Debug, Default, Hash)]
pub struct NopInput;

impl InputContext for NopContext {
    type Input = NopInput;

    fn to_bytes<'a>(&mut self, _input: &'a NopInput) -> OwnedSlice<'a, u8> {
        OwnedSlice::from(vec![])
    }

    fn len<'a>(&self, _input: &'a Self::Input) -> usize {
        0
    }
}

impl NopInput {
    /// Creates a new [`NopInput`]
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Input for NopInput {}

impl HasLen for NopInput {
    fn len(&self) -> usize {
        0
    }
}

impl HasMutatorBytes for Vec<u8> {
    fn mutator_bytes(&self) -> &[u8] {
        self.as_ref()
    }

    fn mutator_bytes_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl HasMutatorBytes for &'_ mut Vec<u8> {
    fn mutator_bytes(&self) -> &[u8] {
        self
    }

    fn mutator_bytes_mut(&mut self) -> &mut [u8] {
        self
    }
}

impl<T> ResizableMutator<T> for Vec<T>
where
    T: Copy + 'static,
{
    fn resize(&mut self, new_len: usize, value: T) {
        <Vec<T>>::resize(self, new_len, value);
    }

    fn extend<'a, I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        <Vec<T> as Extend<I::Item>>::extend(self, iter);
    }

    fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        <Vec<T>>::splice(self, range, replace_with)
    }

    fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        <Vec<T>>::drain(self, range)
    }
}

impl ResizableMutator<u8> for &mut Vec<u8> {
    fn resize(&mut self, new_len: usize, value: u8) {
        self.deref_mut().resize(new_len, value);
    }

    fn extend<'b, I: IntoIterator<Item = &'b u8>>(&mut self, iter: I) {
        <Vec<u8> as Extend<I::Item>>::extend(self, iter);
    }

    fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = u8>,
    {
        self.deref_mut().splice::<R, I>(range, replace_with)
    }

    fn drain<R>(&mut self, range: R) -> Drain<'_, u8>
    where
        R: RangeBounds<usize>,
    {
        self.deref_mut().drain(range)
    }
}

#[cfg(test)]
mod tests {
    use libaflmm_bolts::AsSlice;

    use crate::inputs::{BytesInput, InputContext, bytes::BytesContext};

    #[test]
    fn test_from_target_bytes() {
        let original_bytes = vec![0, 1, 2, 3];
        let input = BytesInput::from(original_bytes.clone());
        let mut nop = BytesContext;
        let res = nop.to_bytes(&input);
        assert_eq!(res.as_slice(), &original_bytes);
    }
}
