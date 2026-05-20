//! Custom name hook.

use alloc::string::String;
use core::marker::PhantomData;

use crate::{
    Result,
    common::DependencyResolver,
    corpus::testcase::{Testcase, TestcaseFilenameFormat},
    fuzzers::{FuzzerHook, Verdict},
    runtimes::RuntimeHandle,
};

/// Set a custom filename to the [`Testcase`].
#[derive(Debug)]
pub struct CustomNameHook<I, G, S> {
    name_generator: G,
    phantom: PhantomData<(I, S)>,
}

impl<I, G, S> CustomNameHook<I, G, S> {
    /// Create a new [`CustomNameHook`].
    pub fn new(generator: G) -> Self {
        Self {
            name_generator: generator,
            phantom: PhantomData,
        }
    }
}

/// Type which can generate a custom filename for a given input/state pair
pub trait FilenameGenerator<I, S> {
    /// Sets the name of the provided [`Testcase`] based on the state and input
    fn set_name(&mut self, state: &mut S, testcase: &mut Testcase<I>) -> Result<String>;
}

// maintain compatibility with old impls
impl<I, F, S> FilenameGenerator<I, S> for F
where
    F: FnMut(&mut S, &mut Testcase<I>) -> Result<String>,
{
    fn set_name(&mut self, state: &mut S, testcase: &mut Testcase<I>) -> Result<String> {
        self(state, testcase)
    }
}

impl<I, G, S> DependencyResolver for CustomNameHook<I, G, S> {}

impl<E, I, G, S, W> FuzzerHook<E, I, S, W> for CustomNameHook<I, G, S>
where
    G: FilenameGenerator<I, S>,
{
    fn pre_add(
        &mut self,
        _executor: &mut E,
        state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
        _verdict: Verdict,
    ) -> Result<()> {
        let fmt = TestcaseFilenameFormat::Custom(self.name_generator.set_name(state, testcase)?);
        testcase.set_filename_fmt(fmt);
        Ok(())
    }
}
