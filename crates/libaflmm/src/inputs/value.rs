//! Newtype pattern style wrapper for [`Input`]s
//! This allows us to wrap common types as [`Input`], such as [`alloc::vec::Vec<u8>`] as [`crate::inputs::BytesInput`] and use those for mutations.

use alloc::vec::Vec;
use core::{fmt::Debug, hash::Hash, marker::PhantomData};

use libafl_bolts::{Error, ownedref::OwnedSlice, rands::Rand};
use serde::{Deserialize, Serialize};
use {
    libafl_bolts::fs::write_file_atomic,
    std::{fs::File, io::Read, path::Path},
};

use crate::inputs::{Input, InputContext};

/// Functionality required for Numeric Mutators (see [`int_mutators`])
pub trait Numeric {
    /// Flip all bits of the number.
    fn flip_all_bits(&mut self);

    /// Flip the bit at the specified offset.
    ///
    /// # Safety
    ///
    /// Panics if the `offset` is out of bounds for the type
    fn flip_bit_at(&mut self, offset: usize);

    /// Increment the number by one, wrapping around on overflow.
    fn wrapping_inc(&mut self);

    /// Decrement the number by one, wrapping around on underflow.
    fn wrapping_dec(&mut self);

    /// Compute the two's complement of the number.
    fn twos_complement(&mut self);

    /// Randomizes the value using the provided random number generator.
    fn randomize<R: Rand>(&mut self, rand: &mut R);
}

/// A wrapper that implements [`FromTargetBytesConverter`] for [`ValueInput`] of primitives
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PrimitiveContext<T> {
    phantom: PhantomData<T>,
}

/// Newtype pattern wrapper around an underlying structure to implement inputs
///
/// This does not blanket implement [`super::Input`], because for certain inputs, writing them to disk does not make sense, because they don't own their data (like [`super::MutVecInput`])
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub struct ValueInput<T>(T);

impl<T> PrimitiveContext<T> {
    /// Creates a new [`PrimitiveInputConverter`]
    #[must_use]
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T> From<T> for ValueInput<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> ValueInput<T> {
    /// Create a new [`ValueInput`]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Extract the inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> AsRef<T> for ValueInput<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for ValueInput<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Copy> Copy for ValueInput<T> {}

// Macro to implement the `Input` trait and create type aliases for `WrappingInput<T>`
macro_rules! impl_input_for_value_input {
    ($($t:ty => $name:ident),+ $(,)?) => {
        $(
            impl Input for ValueInput<$t> {
            }

            /// Input wrapping a <$t>
            pub type $name = ValueInput<$t>;
        )*
    };
}

// Invoke the macro with type-name pairs
impl_input_for_value_input!(
    u8 => U8Input,
    u16 => U16Input,
    u32 => U32Input,
    u64 => U64Input,
    u128 => U128Input,
    usize => UsizeInput,
    i8 => I8Input,
    i16 => I16Input,
    i32 => I32Input,
    i64 => I64Input,
    i128 => I128Input,
    isize => IsizeInput,
);

macro_rules! impl_to_bytes_for_primitive {
    ($($t:ty),+ $(,)?) => {
        $(
            impl InputContext<ValueInput<$t>> for PrimitiveContext<$t> {
                fn to_bytes<'a>(&mut self, input: &'a ValueInput<$t>) -> OwnedSlice<'a, u8> {
                    OwnedSlice::from(input.into_inner().to_le_bytes().to_vec())
                }
            }
        )*
    };
}
impl_to_bytes_for_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);
// Macro to implement the Numeric trait for multiple integer types a u64 can be cast to
macro_rules! impl_numeric_cast_randomize {
    ($($t:ty)*) => ($(
        impl Numeric for $t {
            #[inline]
            fn flip_all_bits(&mut self) {
                *self = !*self;
            }

            #[inline]
            fn flip_bit_at(&mut self, offset: usize) {
                *self ^= 1 << offset;
            }

            #[inline]
            fn wrapping_inc(&mut self) {
                *self = self.wrapping_add(1);
            }

            #[inline]
            fn wrapping_dec(&mut self) {
                *self = self.wrapping_sub(1);
            }

            #[inline]
            fn twos_complement(&mut self) {
                *self = self.wrapping_neg();
            }

            #[inline]
            #[allow(trivial_numeric_casts, clippy::cast_possible_wrap)] // only for some macro calls
            fn randomize<R: Rand>(&mut self, rand: &mut R) {
                *self = rand.next() as $t;
            }

        }
    )*)
}
impl_numeric_cast_randomize!( u8 u16 u32 u64 usize i8 i16 i32 i64 isize );

/// manually implemented because files can be written more efficiently
impl Input for ValueInput<Vec<u8>> {
    /// Write this input to the file
    fn to_file<P>(&self, path: P) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        write_file_atomic(path, self.as_ref())?;
        Ok(())
    }

    /// Load the content of this input from a file
    fn from_file<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let mut file = File::open(path)?;
        let mut data = vec![];
        file.read_to_end(&mut data)?;
        Ok(data.into())
    }
}

impl<T> Numeric for ValueInput<T>
where
    T: Numeric,
{
    fn flip_all_bits(&mut self) {
        self.as_mut().flip_all_bits();
    }

    fn flip_bit_at(&mut self, rhs: usize) {
        self.as_mut().flip_bit_at(rhs);
    }

    fn wrapping_inc(&mut self) {
        self.as_mut().wrapping_inc();
    }

    fn wrapping_dec(&mut self) {
        self.as_mut().wrapping_dec();
    }

    fn twos_complement(&mut self) {
        self.as_mut().twos_complement();
    }

    fn randomize<R: Rand>(&mut self, rand: &mut R) {
        self.as_mut().randomize(rand);
    }
}

// Macro to implement the Numeric trait for multiple integer types a u64 cannot be cast to
macro_rules! impl_numeric_128_bits_randomize {
    ($($t:ty)*) => ($(
        impl Numeric for $t {
            #[inline]
            fn flip_all_bits(&mut self) {
                *self = !*self;
            }

            #[inline]
            fn flip_bit_at(&mut self, offset: usize) {
                *self ^= 1 << offset;
            }

            #[inline]
            fn wrapping_inc(&mut self) {
                *self = self.wrapping_add(1);
            }

            #[inline]
            fn wrapping_dec(&mut self) {
                *self = self.wrapping_sub(1);
            }

            #[inline]
            fn twos_complement(&mut self) {
                *self = self.wrapping_neg();
            }

            #[inline]
            #[allow(trivial_numeric_casts, clippy::cast_possible_wrap)] // only for some macro calls
            fn randomize<R: Rand>(&mut self, rand: &mut R) {
                *self = (u128::from(rand.next()) << 64 | u128::from(rand.next())) as $t;
            }

        }
    )*)
}

// Apply the macro to all desired integer types
impl_numeric_128_bits_randomize! { u128 i128 }

impl<I: Numeric> Numeric for &mut I {
    fn flip_all_bits(&mut self) {
        (*self).flip_all_bits();
    }

    fn flip_bit_at(&mut self, offset: usize) {
        (*self).flip_bit_at(offset);
    }

    fn wrapping_inc(&mut self) {
        (*self).wrapping_inc();
    }

    fn wrapping_dec(&mut self) {
        (*self).wrapping_dec();
    }

    fn twos_complement(&mut self) {
        (*self).twos_complement();
    }

    fn randomize<R: Rand>(&mut self, rand: &mut R) {
        (*self).randomize(rand);
    }
}

#[cfg(test)]
mod tests {
    use core::{any::type_name, fmt::Debug};

    use super::ValueInput;
    use crate::inputs::value::Numeric;

    macro_rules! apply_all_ops {
        ($prep:stmt, $value:expr, $type:ty, $check_twos_complement:expr) => {{
            $prep
            let mut j = $value;
            j.flip_all_bits();
            $prep
            assert_ne!(j, $value, "{:?}.flip_all_bits() for {}", j, type_name::<$type>());

            $prep
            let mut j = $value;
            j.wrapping_inc();
            $prep
            assert_ne!(j, $value, "{:?}.wrapping_inc() for {}", j, type_name::<$type>());

            $prep
            let mut j = $value;
            j.wrapping_dec();
            $prep
            assert_ne!(j, $value, "{:?}.wrapping_dec() for {}", j, type_name::<$type>());

            $prep
            let mut j = $value;
            j.twos_complement();
            if $check_twos_complement {
                $prep
                assert_ne!(j, $value, "{:?}.twos_complement() for {}", j, type_name::<$type>());
            }

            $prep
            let mut j = $value;
            j.flip_bit_at(0);
            $prep
            assert_ne!(j, $value, "{:?}.flip_bit_at(0) for {}", j, type_name::<$type>());

            $prep
            let mut j = $value;
            j.flip_bit_at(size_of::<T>() * 8 - 1);
            $prep
            assert_ne!(j, $value, "{:?}.flip_bit_at({}) for {}", j, size_of::<T>() * 8 - 1, type_name::<$type>());
        }};
    }

    #[expect(unused_mut)]
    fn take_numeric<T: Numeric + Clone + PartialEq + Debug>(val: &T, check_twos_complement: bool) {
        apply_all_ops!({}, val.clone(), T, check_twos_complement);
        apply_all_ops!(
            {},
            ValueInput::from(val.clone()),
            ValueInput<T>,
            check_twos_complement
        );
        apply_all_ops!(
            let mut val_clone = val.clone(),
            &mut val_clone,
            &mut T,
            check_twos_complement
        );
    }

    #[test]
    fn compiles() {
        // twos complement doesn't change anything on the min value of numeric types
        take_numeric(&u8::MIN, false);
        take_numeric(&u16::MIN, false);
        take_numeric(&u32::MIN, false);
        take_numeric(&u64::MIN, false);
        take_numeric(&u128::MIN, false);
        take_numeric(&usize::MIN, false);
        take_numeric(&i8::MIN, false);
        take_numeric(&i16::MIN, false);
        take_numeric(&i32::MIN, false);
        take_numeric(&i64::MIN, false);
        take_numeric(&i128::MIN, false);
        take_numeric(&isize::MIN, false);
        take_numeric(&u8::MAX, true);
        take_numeric(&u16::MAX, true);
        take_numeric(&u32::MAX, true);
        take_numeric(&u64::MAX, true);
        take_numeric(&u128::MAX, true);
        take_numeric(&usize::MAX, true);
        take_numeric(&i8::MAX, true);
        take_numeric(&i16::MAX, true);
        take_numeric(&i32::MAX, true);
        take_numeric(&i64::MAX, true);
        take_numeric(&i128::MAX, true);
        take_numeric(&isize::MAX, true);
    }
}
