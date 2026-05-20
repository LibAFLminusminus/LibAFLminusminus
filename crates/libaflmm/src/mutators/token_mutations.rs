//! Tokens are what AFL calls extras or dictionaries.
//! They may be inserted as part of mutations during fuzzing.
use alloc::{borrow::Cow, vec::Vec};
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
use core::slice::from_raw_parts;
use core::{
    fmt::Debug,
    num::NonZero,
    ops::{Add, AddAssign, Deref},
    slice::Iter,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use hashbrown::HashSet;
use libaflmm_bolts::{AsSlice, rands::Rand};
use serde::{Deserialize, Serialize};

use crate::mutators::str_decode;
use crate::{
    Error, EvaluationResult,
    inputs::{HasMutatorBytes, ResizableMutator},
    mutators::{MutationResult, Mutator, Named, buffer_self_copy, mutations::buffer_copy},
    states::State,
};

/// A state metadata holding a list of tokens
#[expect(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tokens {
    // We keep a vec and a set, set for faster deduplication, vec for access
    tokens_vec: Vec<Vec<u8>>,
    tokens_set: HashSet<Vec<u8>>,
}

libaflmm_bolts::impl_serdeany!(Tokens);

/// The default metadata name for holding the tokens map
pub static DEFAULT_TOKEN_MAP: &str = "default_tokens";

/// The metadata used for token mutators
impl Tokens {
    /// Creates a new [`Tokens`] metadata (old-skool afl name: `dictornary`)
    #[must_use]
    pub fn new() -> Self {
        Tokens::default()
    }

    /// Add [`Tokens`] from a slice of Vecs of bytes
    pub fn add_tokens<IT, V>(&mut self, tokens: IT) -> &mut Self
    where
        IT: IntoIterator<Item = V>,
        V: AsRef<Vec<u8>>,
    {
        for token in tokens {
            self.add_token(token.as_ref());
        }
        self
    }

    /// Build [`Tokens`] from files
    pub fn add_from_files<IT, P>(mut self, files: IT) -> Result<Self, Error>
    where
        IT: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for file in files {
            self.add_from_file(file)?;
        }
        Ok(self)
    }

    /// Parse autodict section
    pub fn parse_autodict(&mut self, slice: &[u8], size: usize) {
        let mut head = 0;
        loop {
            if head >= size {
                // Make double sure this is not completely off
                assert!(head == size);
                break;
            }
            let size = slice[head] as usize;
            head += 1;
            if size > 0 {
                self.add_token(&slice[head..head + size].to_vec());
                log::info!(
                    "Token size: {} content: {:x?}",
                    size,
                    &slice[head..head + size].to_vec()
                );
                head += size;
            }
        }
    }

    /// Create a [`Tokens`] section from a start and an end pointer
    /// Reads from an autotokens section, returning the count of new entries read
    ///
    /// # Safety
    /// The caller must ensure that the region between `token_start` and `token_stop`
    /// is a valid region, containing autotokens in the expected format.
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    pub unsafe fn from_mut_ptrs(
        token_start: *const u8,
        token_stop: *const u8,
    ) -> Result<Self, Error> {
        unsafe {
            let mut ret = Self::default();
            if token_start.is_null() || token_stop.is_null() {
                return Ok(Self::new());
            }
            if token_stop < token_start {
                return Err(Error::illegal_argument(format!(
                    "Tried to create tokens from illegal section: stop < start ({token_stop:?} < {token_start:?})"
                )));
            }
            let section_size: usize = token_stop.offset_from(token_start).try_into().unwrap();
            // log::info!("size: {}", section_size);
            let slice = from_raw_parts(token_start, section_size);

            // Now we know the beginning and the end of the token section.. let's parse them into tokens
            ret.parse_autodict(slice, section_size);

            Ok(ret)
        }
    }

    /// Creates a new [`Tokens`] from a file
    pub fn from_file<P>(file: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let mut ret = Self::new();
        ret.add_from_file(file)?;
        Ok(ret)
    }

    /// Adds a vector of [`Tokens`] to a dictionary, checking it is not a duplicate
    /// Returns `false` if the token was already present and did not get added.
    #[expect(clippy::ptr_arg)]
    pub fn add_token(&mut self, token: &Vec<u8>) -> bool {
        if !self.tokens_set.insert(token.clone()) {
            return false;
        }
        self.tokens_vec.push(token.clone());
        true
    }

    /// Reads a [`Tokens`] file, returning the count of new entries read
    pub fn add_from_file<P>(&mut self, file: P) -> Result<&mut Self, Error>
    where
        P: AsRef<Path>,
    {
        // log::info!("Loading tokens file {:?} ...", file);

        let file = File::open(file)?; // panic if not found
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.unwrap();
            let line = line.trim_start().trim_end();

            // we are only interested in '"..."', not prefixed 'foo = '
            let start = line.chars().next();
            if line.is_empty() || start == Some('#') {
                continue;
            }
            let Some(pos_quote) = line.find('\"') else {
                return Err(Error::illegal_argument(format!("Illegal line: {line}")));
            };
            if line.chars().nth(line.len() - 1) != Some('"') {
                return Err(Error::illegal_argument(format!("Illegal line: {line}")));
            }

            // extract item
            let Some(item) = line.get(pos_quote + 1..line.len() - 1) else {
                return Err(Error::illegal_argument(format!("Illegal line: {line}")));
            };
            if item.is_empty() {
                continue;
            }

            // decode
            let token: Vec<u8> = match str_decode(item) {
                Ok(val) => val,
                Err(_) => {
                    return Err(Error::illegal_argument(format!(
                        "Illegal line (hex decoding): {line}"
                    )));
                }
            };

            // add
            self.add_token(&token);
        }

        Ok(self)
    }

    /// Returns the amount of tokens in this [`Tokens`] instance
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens_vec.len()
    }

    /// Returns if this [`Tokens`] instance is empty
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens_vec.is_empty()
    }

    /// Gets the [`Tokens`] stored in this db
    #[must_use]
    pub fn tokens(&self) -> &[Vec<u8>] {
        &self.tokens_vec
    }

    /// Returns an iterator over the [`Tokens`].
    pub fn iter(&self) -> Iter<'_, Vec<u8>> {
        <&Self as IntoIterator>::into_iter(self)
    }
}

impl AddAssign for Tokens {
    fn add_assign(&mut self, other: Self) {
        self.add_tokens(&other);
    }
}

impl AddAssign<&[Vec<u8>]> for Tokens {
    fn add_assign(&mut self, other: &[Vec<u8>]) {
        self.add_tokens(other);
    }
}

impl Add<&[Vec<u8>]> for Tokens {
    type Output = Self;
    fn add(self, other: &[Vec<u8>]) -> Self {
        let mut ret = self;
        ret.add_tokens(other);
        ret
    }
}

impl Add for Tokens {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.add(other.tokens_vec.as_slice())
    }
}

impl<IT, V> From<IT> for Tokens
where
    IT: IntoIterator<Item = V>,
    V: AsRef<Vec<u8>>,
{
    fn from(tokens: IT) -> Self {
        let mut ret = Self::default();
        ret.add_tokens(tokens);
        ret
    }
}

impl Deref for Tokens {
    type Target = [Vec<u8>];
    fn deref(&self) -> &[Vec<u8>] {
        self.tokens()
    }
}

impl Add for &Tokens {
    type Output = Tokens;

    fn add(self, other: Self) -> Tokens {
        let mut ret: Tokens = self.clone();
        ret.add_tokens(other);
        ret
    }
}

impl<'it> IntoIterator for &'it Tokens {
    type Item = <Iter<'it, Vec<u8>> as Iterator>::Item;
    type IntoIter = Iter<'it, Vec<u8>>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Inserts a random [`Tokens`] at a random position in the `Input`.
#[derive(Debug, Default)]
pub struct TokenInsert;

impl<I, R, S> Mutator<I, R, S> for TokenInsert
where
    R: Rand,
    S: State,
    I: ResizableMutator<u8> + HasMutatorBytes,
{
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        let max_size = state.max_size();
        let tokens_len = {
            let Some(meta) = state.metadata_map().get::<Tokens>(DEFAULT_TOKEN_MAP) else {
                return Ok(MutationResult::Skipped);
            };
            if let Some(tokens_len) = NonZero::new(meta.tokens().len()) {
                tokens_len
            } else {
                return Ok(MutationResult::Skipped);
            }
        };
        let token_idx = rand.below(tokens_len);

        let size = input.mutator_bytes().len();
        // # Safety
        // after saturating add it's always above 0

        let off = rand.below(unsafe { NonZero::new_unchecked(size.saturating_add(1)) });

        let meta = state
            .metadata_map()
            .get::<Tokens>(DEFAULT_TOKEN_MAP)
            .unwrap();
        let token = &meta.tokens()[token_idx];
        let mut len = token.len();

        if size + len > max_size {
            if max_size > size {
                len = max_size - size;
            } else {
                return Ok(MutationResult::Skipped);
            }
        }

        input.resize(size + len, 0);
        unsafe {
            buffer_self_copy(input.mutator_bytes_mut(), off, off + len, size - off);
            buffer_copy(input.mutator_bytes_mut(), token, 0, off, len);
        }

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for TokenInsert {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("TokenInsert");
        &NAME
    }
}

impl TokenInsert {
    /// Create a [`TokenInsert`] mutation.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// A [`TokenReplace`] [`Mutator`] replaces a random part of the input with one of a range of tokens.
/// From AFL terms, this is called as `Dictionary` mutation (which doesn't really make sense ;) ).
#[derive(Debug, Default)]
pub struct TokenReplace;

impl<I, R, S> Mutator<I, R, S> for TokenReplace
where
    R: Rand,
    S: State,
    I: ResizableMutator<u8> + HasMutatorBytes,
{
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        let size = input.mutator_bytes().len();
        let off = if let Some(nz) = NonZero::new(size) {
            rand.below(nz)
        } else {
            return Ok(MutationResult::Skipped);
        };

        let tokens_len = {
            let Some(meta) = state.metadata_map().get::<Tokens>(DEFAULT_TOKEN_MAP) else {
                return Ok(MutationResult::Skipped);
            };
            if let Some(tokens_len) = NonZero::new(meta.tokens().len()) {
                tokens_len
            } else {
                return Ok(MutationResult::Skipped);
            }
        };
        let token_idx = rand.below(tokens_len);

        let meta = state
            .metadata_map()
            .get::<Tokens>(DEFAULT_TOKEN_MAP)
            .unwrap();
        let token = &meta.tokens()[token_idx];
        let mut len = token.len();
        if off + len > size {
            len = size - off;
        }

        unsafe {
            buffer_copy(input.mutator_bytes_mut(), token, 0, off, len);
        }

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for TokenReplace {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("TokenReplace");
        &NAME
    }
}

impl TokenReplace {
    /// Creates a new [`TokenReplace`] struct.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}
