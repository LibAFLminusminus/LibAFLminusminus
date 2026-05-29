/*!
 * `LibAFL_core` contains core traits used across all crates, including the [`Error`] enum and various traits.
 */
#![doc = include_str!("../README.md")]
/*! */
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use alloc::{borrow::Cow, vec::Vec};
use core::{
    array::TryFromSliceError,
    fmt::{self, Display},
    num::{ParseIntError, TryFromIntError},
    ops::{Deref, DerefMut},
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{env::VarError, io};
use {
    alloc::string::{FromUtf8Error, String},
    core::cell::{BorrowError, BorrowMutError},
    core::str::Utf8Error,
};

pub mod forkserver;

pub mod nonzero_macros;

pub extern crate alloc;

/// The client ID for various use cases across `LibAFL`
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct WorkerId(pub u32);

#[cfg(feature = "errors_backtrace")]
/// Error Backtrace type when `errors_backtrace` feature is enabled (== [`Backtrace`](std::backtrace::Backtrace`))
pub type ErrorBacktrace = std::backtrace::Backtrace;

#[cfg(not(feature = "errors_backtrace"))]
#[derive(Debug, Default)]
/// ZST to use when `errors_backtrace` is disabled
pub struct ErrorBacktrace;

#[cfg(not(feature = "errors_backtrace"))]
impl ErrorBacktrace {
    /// Nop
    #[must_use]
    pub fn capture() -> Self {
        Self
    }
}

/// Appends an [`ErrorBacktrace`] to a formatter, if one was captured.
#[cfg(feature = "errors_backtrace")]
pub fn display_error_backtrace(f: &mut fmt::Formatter, err: &ErrorBacktrace) -> fmt::Result {
    match err.status() {
        std::backtrace::BacktraceStatus::Captured => write!(f, "\nBacktrace:\n{err}"),
        std::backtrace::BacktraceStatus::Disabled => {
            write!(f, "\nRun with `RUST_BACKTRACE=1` to see a backtrace.")
        }
        _ => Ok(()),
    }
}

/// Appends an [`ErrorBacktrace`] to a formatter, if one was captured.
#[cfg(not(feature = "errors_backtrace"))]
pub fn display_error_backtrace(_f: &mut fmt::Formatter, _err: &ErrorBacktrace) -> fmt::Result {
    Ok(())
}

/// Shorthand for `std::result::Result<T, libafl_core::Error>`.
pub type Result<T> = core::result::Result<T, Error>;

/// Main error struct for `LibAFL`
pub enum Error {
    /// Serialization error
    Serialize(String, ErrorBacktrace),
    /// Compression error
    Compression(ErrorBacktrace),
    /// Optional val was supposed to be set, but isn't.
    EmptyOptional(String, ErrorBacktrace),
    /// Key not in Map
    KeyNotFound(String, ErrorBacktrace),
    /// Key already exists and should not overwrite
    KeyExists(String, ErrorBacktrace),
    /// No elements in the current item
    Empty(String, ErrorBacktrace),
    /// End of iteration
    IteratorEnd(String, ErrorBacktrace),
    /// This is not supported (yet)
    NotImplemented(String, ErrorBacktrace),
    /// You're holding it wrong
    IllegalState(String, ErrorBacktrace),
    /// The argument passed to this method or function is not valid
    IllegalArgument(String, ErrorBacktrace),
    /// The performed action is not supported on the current platform
    Unsupported(String, ErrorBacktrace),
    /// Raise this from a stage to skip the remaining stages for a given input, not really an error.
    SkipRemainingStages,
    /// OS error, wrapping a [`io::Error`]
    OsError(io::Error, String, ErrorBacktrace),
    /// Something else happened
    Unknown(String, ErrorBacktrace),
    /// Error with the corpora
    InvalidCorpus(String, ErrorBacktrace),
    /// Error specific to a runtime like QEMU or Frida
    Runtime(String, ErrorBacktrace),
    /// The `Input` was invalid.
    InvalidInput(String, ErrorBacktrace),
    /// This is an error due to a `LibAFLmm` bug.
    /// Please report it.
    InternalBug(String, ErrorBacktrace),
}

impl Error {
    /// Serialization error
    #[must_use]
    pub fn serialize<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::Serialize(arg.into(), ErrorBacktrace::capture())
    }

    /// Compression error
    #[must_use]
    pub fn compression() -> Self {
        Error::Compression(ErrorBacktrace::capture())
    }

    /// Optional val was supposed to be set, but isn't.
    #[must_use]
    pub fn empty_optional<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::EmptyOptional(arg.into(), ErrorBacktrace::capture())
    }

    /// The `Input` was invalid
    #[must_use]
    pub fn invalid_input<S>(reason: S) -> Self
    where
        S: Into<String>,
    {
        Error::InvalidInput(reason.into(), ErrorBacktrace::capture())
    }

    /// Key not in Map
    #[must_use]
    pub fn key_not_found<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::KeyNotFound(arg.into(), ErrorBacktrace::capture())
    }

    /// Key already exists in Map
    #[must_use]
    pub fn key_exists<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::KeyExists(arg.into(), ErrorBacktrace::capture())
    }

    /// No elements in the current item
    #[must_use]
    pub fn empty<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::Empty(arg.into(), ErrorBacktrace::capture())
    }

    /// End of iteration
    #[must_use]
    pub fn iterator_end<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::IteratorEnd(arg.into(), ErrorBacktrace::capture())
    }

    /// This is not supported (yet)
    #[must_use]
    pub fn not_implemented<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::NotImplemented(arg.into(), ErrorBacktrace::capture())
    }

    /// You're holding it wrong
    #[must_use]
    pub fn illegal_state<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::IllegalState(arg.into(), ErrorBacktrace::capture())
    }

    /// The argument passed to this method or function is not valid
    #[must_use]
    pub fn illegal_argument<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::IllegalArgument(arg.into(), ErrorBacktrace::capture())
    }

    /// This operation is not supported on the current architecture or platform
    #[must_use]
    pub fn unsupported<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::Unsupported(arg.into(), ErrorBacktrace::capture())
    }

    /// OS error with additional message
    #[must_use]
    pub fn os_error<S>(err: io::Error, msg: S) -> Self
    where
        S: Into<String>,
    {
        Error::OsError(err, msg.into(), ErrorBacktrace::capture())
    }

    /// OS error from [`io::Error::last_os_error`] with additional message
    #[must_use]
    pub fn last_os_error<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Error::OsError(
            io::Error::last_os_error(),
            msg.into(),
            ErrorBacktrace::capture(),
        )
    }

    /// Something else happened
    #[must_use]
    pub fn unknown<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::Unknown(arg.into(), ErrorBacktrace::capture())
    }

    /// Error with corpora
    #[must_use]
    pub fn invalid_corpus<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::InvalidCorpus(arg.into(), ErrorBacktrace::capture())
    }

    /// Error specific to some runtime, like QEMU or Frida
    #[must_use]
    pub fn runtime<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::Runtime(arg.into(), ErrorBacktrace::capture())
    }

    /// General `LibAFLmm` bug
    #[must_use]
    pub fn internal_bug<S>(arg: S) -> Self
    where
        S: Into<String>,
    {
        Error::InternalBug(arg.into(), ErrorBacktrace::capture())
    }

    /// Skip the remaining stages for this input
    #[must_use]
    pub fn skip_remaining_stages() -> Self {
        Error::SkipRemainingStages
    }
}

/// build an [`Error::Serialize`].
#[macro_export]
macro_rules! serialize {
    ($($arg:tt)*) => { $crate::Error::serialize(format!($($arg)*)) };
}

/// build an [`Error::EmptyOptional`].
#[macro_export]
macro_rules! empty_optional {
    ($($arg:tt)*) => { $crate::Error::empty_optional(format!($($arg)*)) };
}

/// build an [`Error::InvalidInput`].
#[macro_export]
macro_rules! invalid_input {
    ($($arg:tt)*) => { $crate::Error::invalid_input(format!($($arg)*)) };
}

/// build an [`Error::KeyNotFound`].
#[macro_export]
macro_rules! key_not_found {
    ($($arg:tt)*) => { $crate::Error::key_not_found(format!($($arg)*)) };
}

/// build an [`Error::KeyExists`].
#[macro_export]
macro_rules! key_exists {
    ($($arg:tt)*) => { $crate::Error::key_exists(format!($($arg)*)) };
}

/// build an [`Error::Empty`].
#[macro_export]
macro_rules! empty {
    ($($arg:tt)*) => { $crate::Error::empty(format!($($arg)*)) };
}

/// build an [`Error::IteratorEnd`].
#[macro_export]
macro_rules! iterator_end {
    ($($arg:tt)*) => { $crate::Error::iterator_end(format!($($arg)*)) };
}

/// build an [`Error::NotImplemented`].
#[macro_export]
macro_rules! not_implemented {
    ($($arg:tt)*) => { $crate::Error::not_implemented(format!($($arg)*)) };
}

/// build an [`Error::IllegalState`].
#[macro_export]
macro_rules! illegal_state {
    ($($arg:tt)*) => { $crate::Error::illegal_state(format!($($arg)*)) };
}

/// build an [`Error::IllegalArgument`].
#[macro_export]
macro_rules! illegal_argument {
    ($($arg:tt)*) => { $crate::Error::illegal_argument(format!($($arg)*)) };
}

/// build an [`Error::Unsupported`].
#[macro_export]
macro_rules! unsupported {
    ($($arg:tt)*) => { $crate::Error::unsupported(format!($($arg)*)) };
}

/// build an [`Error::Unknown`].
#[macro_export]
macro_rules! unknown {
    ($($arg:tt)*) => { $crate::Error::unknown(format!($($arg)*)) };
}

/// build an [`Error::InvalidCorpus`].
#[macro_export]
macro_rules! invalid_corpus {
    ($($arg:tt)*) => { $crate::Error::invalid_corpus(format!($($arg)*)) };
}

/// build an [`Error::Runtime`].
#[macro_export]
macro_rules! runtime {
    ($($arg:tt)*) => { $crate::Error::runtime(format!($($arg)*)) };
}

/// build an [`Error::InternalBug`].
#[macro_export]
macro_rules! internal_bug {
    ($($arg:tt)*) => { $crate::Error::internal_bug(format!($($arg)*)) };
}

/// build an [`Error::OsError`] from an [`io::Error`].
#[macro_export]
macro_rules! os_error {
    ($err:expr, $($arg:tt)*) => { $crate::Error::os_error($err, format!($($arg)*)) };
}

/// build an [`Error::OsError`] from [`io::Error::last_os_error`].
#[macro_export]
macro_rules! last_os_error {
    ($($arg:tt)*) => { $crate::Error::last_os_error(format!($($arg)*)) };
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        if let Self::OsError(err, _, _) = self {
            Some(err)
        } else {
            None
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Serialize(s, b) => {
                write!(f, "Error in Serialization: `{0}`", &s)?;
                display_error_backtrace(f, b)
            }
            Self::Compression(b) => {
                write!(f, "Error in decompression")?;
                display_error_backtrace(f, b)
            }
            Self::EmptyOptional(s, b) => {
                write!(f, "Optional value `{0}` was not set", &s)?;
                display_error_backtrace(f, b)
            }
            Self::KeyNotFound(s, b) => {
                write!(f, "Key: `{0}` - not found", &s)?;
                display_error_backtrace(f, b)
            }
            Self::KeyExists(s, b) => {
                write!(f, "Key: `{0}` - already exists", &s)?;
                display_error_backtrace(f, b)
            }
            Self::Empty(s, b) => {
                write!(f, "No items in {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::IteratorEnd(s, b) => {
                write!(f, "All elements have been processed in {0} iterator", &s)?;
                display_error_backtrace(f, b)
            }
            Self::NotImplemented(s, b) => {
                write!(f, "Not implemented: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::IllegalState(s, b) => {
                write!(f, "Illegal state: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::IllegalArgument(s, b) => {
                write!(f, "Illegal argument: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::Unsupported(s, b) => {
                write!(
                    f,
                    "The operation is not supported on the current platform: {0}",
                    &s
                )?;
                display_error_backtrace(f, b)
            }
            Self::OsError(err, s, b) => {
                write!(f, "OS error: {0}: {1}", &s, err)?;
                display_error_backtrace(f, b)
            }
            Self::Unknown(s, b) => {
                write!(f, "Unknown error: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::InvalidCorpus(s, b) => {
                write!(f, "Invalid corpus: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::Runtime(s, b) => {
                write!(f, "Runtime error: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::InvalidInput(s, b) => {
                write!(f, "Encountered an invalid input: {0}", &s)?;
                display_error_backtrace(f, b)
            }
            Self::InternalBug(s, b) => {
                writeln!(f, "LibAFL internal bug: {0}", &s)?;
                write!(
                    f,
                    "This is a LibAFLmm bug, please open an issue at https://github.com/LibAFLminusminus/LibAFLminusminus.",
                )?;
                display_error_backtrace(f, b)
            }
            Self::SkipRemainingStages => write!(f, "Skip remaining stages"),
        }
    }
}

impl From<BorrowError> for Error {
    fn from(err: BorrowError) -> Self {
        crate::illegal_state!("Couldn't borrow from a RefCell as immutable: {err:?}")
    }
}

impl From<BorrowMutError> for Error {
    fn from(err: BorrowMutError) -> Self {
        crate::illegal_state!("Couldn't borrow from a RefCell as mutable: {err:?}")
    }
}

/// Stringify the postcard serializer error
impl From<postcard::Error> for Error {
    fn from(err: postcard::Error) -> Self {
        crate::serialize!("{err:?}")
    }
}

impl From<nix::Error> for Error {
    fn from(err: nix::Error) -> Self {
        crate::unknown!("Unix error: {err:?}")
    }
}

/// Create an AFL Error from io Error
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        crate::os_error!(err, "io::Error ocurred")
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Self {
        crate::unknown!("Could not convert byte / utf-8: {err:?}")
    }
}

impl From<Utf8Error> for Error {
    fn from(err: Utf8Error) -> Self {
        crate::unknown!("Could not convert byte / utf-8: {err:?}")
    }
}

impl From<VarError> for Error {
    fn from(err: VarError) -> Self {
        crate::empty!("Could not get env var: {err:?}")
    }
}

impl From<ParseIntError> for Error {
    #[allow(unused_variables)] // err is unused without std
    fn from(err: ParseIntError) -> Self {
        crate::unknown!("Failed to parse Int: {err:?}")
    }
}

impl From<TryFromIntError> for Error {
    #[allow(unused_variables)] // err is unused without std
    fn from(err: TryFromIntError) -> Self {
        crate::illegal_state!("Expected conversion failed: {err:?}")
    }
}

impl From<TryFromSliceError> for Error {
    #[allow(unused_variables)] // err is unused without std
    fn from(err: TryFromSliceError) -> Self {
        crate::illegal_argument!("Could not convert slice: {err:?}")
    }
}

#[cfg(windows)]
impl From<windows_result::Error> for Error {
    #[allow(unused_variables)] // err is unused without std
    fn from(err: windows_result::Error) -> Self {
        crate::unknown!("Windows API error: {err:?}")
    }
}

// #[cfg(feature = "python")]
// impl From<pyo3::PyErr> for Error {
//     fn from(err: pyo3::PyErr) -> Self {
//         pyo3::Python::attach(|py| {
//             if err
//                 .matches(
//                     py,
//                     pyo3::types::PyType::new::<pyo3::exceptions::PyKeyboardInterrupt>(py),
//                 )
//                 .unwrap()
//             {
//                 Ok(())
//             } else {
//                 Self::illegal_state(format!("Python exception: {err:?}"))
//             }
//         })
//     }
// }

/// Trait to convert into an Owned type
pub trait IntoOwned {
    /// Returns if the current type is an owned type.
    #[must_use]
    fn is_owned(&self) -> bool;

    /// Transfer the current type into an owned type.
    #[must_use]
    fn into_owned(self) -> Self;
}

/// Can be converted to a slice
pub trait AsSlice<'a> {
    /// Type of the entries of this slice
    type Entry: 'a;
    /// Type of the reference to this slice
    type SliceRef: Deref<Target = [Self::Entry]>;

    /// Convert to a slice
    fn as_slice(&'a self) -> Self::SliceRef;
}

/// Can be converted to a slice
pub trait AsSizedSlice<'a, const N: usize> {
    /// Type of the entries of this slice
    type Entry: 'a;
    /// Type of the reference to this slice
    type SliceRef: Deref<Target = [Self::Entry; N]>;

    /// Convert to a slice
    fn as_sized_slice(&'a self) -> Self::SliceRef;
}

impl<'a, T, R: ?Sized> AsSlice<'a> for R
where
    T: 'a,
    R: Deref<Target = [T]>,
{
    type Entry = T;
    type SliceRef = &'a [T];

    fn as_slice(&'a self) -> Self::SliceRef {
        self
    }
}

impl<'a, T, const N: usize, R: ?Sized> AsSizedSlice<'a, N> for R
where
    T: 'a,
    R: Deref<Target = [T; N]>,
{
    type Entry = T;
    type SliceRef = &'a [T; N];

    fn as_sized_slice(&'a self) -> Self::SliceRef {
        self
    }
}

/// Can be converted to a mutable slice
pub trait AsSliceMut<'a>: AsSlice<'a> {
    /// Type of the mutable reference to this slice
    type SliceRefMut: DerefMut<Target = [Self::Entry]>;

    /// Convert to a slice
    fn as_slice_mut(&'a mut self) -> Self::SliceRefMut;
}

/// Can be converted to a mutable slice
pub trait AsSizedSliceMut<'a, const N: usize>: AsSizedSlice<'a, N> {
    /// Type of the mutable reference to this slice
    type SliceRefMut: DerefMut<Target = [Self::Entry; N]>;

    /// Convert to a slice
    fn as_sized_slice_mut(&'a mut self) -> Self::SliceRefMut;
}

impl<'a, T, R: ?Sized> AsSliceMut<'a> for R
where
    T: 'a,
    R: DerefMut<Target = [T]>,
{
    type SliceRefMut = &'a mut [T];

    fn as_slice_mut(&'a mut self) -> Self::SliceRefMut {
        &mut *self
    }
}

impl<'a, T, const N: usize, R: ?Sized> AsSizedSliceMut<'a, N> for R
where
    T: 'a,
    R: DerefMut<Target = [T; N]>,
{
    type SliceRefMut = &'a mut [T; N];

    fn as_sized_slice_mut(&'a mut self) -> Self::SliceRefMut {
        &mut *self
    }
}

/// Create an `Iterator` from a reference
pub trait AsIter<'it> {
    /// The item type
    type Item: 'it;
    /// The ref type
    type Ref: Deref<Target = Self::Item>;
    /// The iterator type
    type IntoIter: Iterator<Item = Self::Ref>;

    /// Create an iterator from &self
    fn as_iter(&'it self) -> Self::IntoIter;
}

impl<'it, S, T> AsIter<'it> for S
where
    S: AsSlice<'it, Entry = T, SliceRef = &'it [T]>,
    T: 'it,
{
    type Item = S::Entry;
    type Ref = &'it Self::Item;
    type IntoIter = core::slice::Iter<'it, Self::Item>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Create an `Iterator` from a mutable reference
pub trait AsIterMut<'it>: AsIter<'it> {
    /// The ref type
    type RefMut: DerefMut<Target = Self::Item>;
    /// The iterator type
    type IntoIterMut: Iterator<Item = Self::RefMut>;

    /// Create an iterator from &mut self
    fn as_iter_mut(&'it mut self) -> Self::IntoIterMut;
}

impl<'it, S, T> AsIterMut<'it> for S
where
    S: AsSliceMut<'it, Entry = T, SliceRef = &'it [T], SliceRefMut = &'it mut [T]>,
    T: 'it,
{
    type RefMut = &'it mut Self::Item;
    type IntoIterMut = core::slice::IterMut<'it, Self::Item>;

    fn as_iter_mut(&'it mut self) -> Self::IntoIterMut {
        self.as_slice_mut().iter_mut()
    }
}

/// Has a ref count
pub trait HasRefCnt {
    /// The ref count
    fn refcnt(&self) -> isize;
    /// The ref count, mutable
    fn refcnt_mut(&mut self) -> &mut isize;
}

/// Create a [`Vec`] of the given type with `nb_elts` elements, initialized in place.
/// The closure must initialize [`Vec`] (of size `nb_elts` * `sizeo_of::<T>()`).
///
/// # Safety
///
/// The input closure should fully initialize the new [`Vec`], not leaving any uninitialized bytes.
// TODO: Use MaybeUninit API at some point.
#[expect(clippy::uninit_vec)]
pub unsafe fn vec_init<E, F, T>(nb_elts: usize, init_fn: F) -> core::result::Result<Vec<T>, E>
where
    F: FnOnce(&mut Vec<T>) -> core::result::Result<(), E>,
{
    unsafe {
        let mut new_vec: Vec<T> = Vec::with_capacity(nb_elts);
        new_vec.set_len(nb_elts);

        init_fn(&mut new_vec)?;

        Ok(new_vec)
    }
}

/// We need fixed names for many parts of this lib.
pub trait Named {
    /// Provide the name of this element.
    fn name(&self) -> &Cow<'static, str>;
}

impl Named for () {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("()");
        &NAME
    }
}

impl<N> Named for Option<N>
where
    N: Named,
{
    fn name(&self) -> &Cow<'static, str> {
        if let Some(named) = self {
            named.name()
        } else {
            static EMPTY: Cow<'static, str> = Cow::Borrowed("<empty>");
            &EMPTY
        }
    }
}

/// Has a length field
pub trait HasLen {
    /// The length
    fn len(&self) -> usize;

    /// Returns `true` if it has no elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> HasLen for Vec<T> {
    #[inline]
    fn len(&self) -> usize {
        Vec::<T>::len(self)
    }
}

impl<T: HasLen> HasLen for &mut T {
    fn len(&self) -> usize {
        self.deref().len()
    }
}

impl<Head, Tail> HasLen for (Head, Tail)
where
    Tail: HasLen,
{
    #[inline]
    fn len(&self) -> usize {
        self.1.len() + 1
    }
}

impl<Tail> HasLen for (Tail,)
where
    Tail: HasLen,
{
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl HasLen for () {
    #[inline]
    fn len(&self) -> usize {
        0
    }
}

/// Trait to truncate slices and maps to a new size
pub trait Truncate {
    /// Reduce the size of the slice
    fn truncate(&mut self, len: usize);
}

impl<T> Truncate for &[T] {
    fn truncate(&mut self, len: usize) {
        *self = &self[..len];
    }
}

impl<T> Truncate for &mut [T] {
    fn truncate(&mut self, len: usize) {
        let value = core::mem::take(self);
        let len = value.len().min(len);
        let truncated = value
            .get_mut(..len)
            .expect("Truncate with len <= len() should always work");
        let _: &mut [T] = core::mem::replace(self, truncated);
    }
}
