#![no_std]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![doc = include_str!("../README.md")]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(missing_docs)]
#![deny(unused_must_use)]

/// Zero-cost way to construct `NonZero*` types from [`core::num`] at compile-time.
///
/// This macro ensures that the value is non-zero at compile-time and returns the corresponding `NonZero` type.
/// If the value is zero, it will cause a compile-time panic.
#[macro_export]
macro_rules! non_zero {
    ($val:expr) => {
        core::num::NonZero::new($val).unwrap()
    };
}

/// Zero-cost way to construct `NonZero*` types from [`core::num`] at compile-time.
///
/// This macro ensures that the value is non-zero at compile-time and returns the corresponding `NonZero` type.
/// If the value is zero, it will cause a compile-time panic.
#[macro_export]
macro_rules! non_zero_unchecked {
    ($val:expr) => {
        unsafe { core::num::NonZero::new_unchecked($val).unwrap() }
    };
}

/// Zero-cost way to construct `NonZero*` types from [`core::num`] at compile-time.
///
/// This macro ensures that the value is non-zero at compile-time and returns the corresponding `NonZero` type.
/// If the value is zero, it will cause a compile-time panic.
#[macro_export]
macro_rules! non_zero_const {
    ($val:expr) => {
        const { core::num::NonZero::new($val).unwrap() }
    };
}

/// Construct `Option<NonZero*>` types from [`core::num`].
///
/// This macro creates an `Option<NonZero*>` from a value.
/// It works both at compile-time (in const contexts) and at runtime.
#[macro_export]
macro_rules! try_non_zero {
    ($val:expr) => {
        core::num::NonZero::new($val)
    };
}

/// Get a [`core::ptr::NonNull`] to a global static mut (or similar).
///
/// The same as [`core::ptr::addr_of_mut`] or `&raw mut`, but wrapped in said [`NonNull`](core::ptr::NonNull).
#[macro_export]
macro_rules! nonnull_raw_mut {
    ($val:expr) => {{
        let ptr = &raw mut $val;
        assert!(
            !ptr.is_null(),
            "Pointer to value was null in `nonnull_raw_mut!`"
        );
        // # Safety
        // The pointer is checked to be non-null by the assertion above.
        unsafe { core::ptr::NonNull::new_unchecked(ptr) }
    }};
}

#[cfg(test)]
mod tests {
    use core::num::{NonZeroU8, NonZeroUsize};

    #[test]
    fn test_nonzero() {
        const VAL: NonZeroUsize = non_zero!(10);
        assert_eq!(VAL.get(), 10);
    }

    #[test]
    fn test_try_nonzero() {
        // Const context
        const VAL: Option<NonZeroU8> = try_non_zero!(5);
        const ZERO: Option<NonZeroU8> = try_non_zero!(0);

        assert!(VAL.is_some());
        assert_eq!(VAL.unwrap().get(), 5);
        assert!(ZERO.is_none());

        // Runtime context
        let x = 5;
        let val: Option<NonZeroU8> = try_non_zero!(x);
        assert!(val.is_some());
        assert_eq!(val.unwrap().get(), 5);
    }

    #[test]
    fn test_nonnull_raw_mut() {
        static mut VAL: usize = 0;
        let ptr = nonnull_raw_mut!(VAL);
        unsafe {
            *ptr.as_ptr() = 123;
            assert_eq!(*ptr.as_ptr(), 123);
        }
    }
}
