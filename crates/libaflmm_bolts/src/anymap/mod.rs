//! A map that can retrieve values by type - and is `Serde` serializable.
//! Poor-rust-man's downcasts to have `AnyMap`

use core::{any::TypeId, mem::size_of, ptr::read_unaligned};

pub mod serdeany;
pub use serdeany::{
    EMPTY_MAP_KEY, NamedSerdeAnyMap, SerdeAny, SerdeAnyMap, add_named_metadata, named_metadata,
    named_metadata_mut, unnamed_metadata, unnamed_metadata_mut,
};

/// Unpack a [`TypeId`] to an `u128`
/// Opposite of [`pack_type_id`].
///
/// # Note
/// Probably not safe for future compilers, fine for now.
/// The size changed in later rust versions, see <https://github.com/rust-lang/compiler-team/issues/608>
#[inline]
#[must_use]
pub const fn unpack_type_id(id: TypeId) -> u128 {
    // see any.rs, it's alway u128 hence 16 bytes.
    // TypeId size of other sizes is not yet supported"
    static_assertions::const_assert!(size_of::<TypeId>() == 16);
    let ret: u128 = unsafe { read_unaligned::<u128>(&raw const id as *const u128) };
    ret
}

#[cfg(test)]
mod test {
    use core::any::TypeId;

    use super::unpack_type_id;

    #[test]
    fn test_type_id() {
        let type_id_u64 = unpack_type_id(TypeId::of::<u64>());
        let type_id_u128 = unpack_type_id(TypeId::of::<u128>());

        assert_ne!(type_id_u64, type_id_u128);
        assert_eq!(type_id_u64, unpack_type_id(TypeId::of::<u64>()));
        assert_eq!(type_id_u128, unpack_type_id(TypeId::of::<u128>()));
    }
}
