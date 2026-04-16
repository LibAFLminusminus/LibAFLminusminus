//! The `TestcaseScore` is an evaluator providing scores of corpus items.
use libafl_bolts::HasLen;

use crate::{Error, corpus::Testcase, state::HasTestcase};

/// Compute the favor factor of a [`Testcase`]. Higher is better.
pub trait TestcaseScore<I, SC> {
    /// Computes the favor factor of a [`Testcase`]. Higher is better.
    fn compute(scheduler: SC, entry: &Testcase<I>) -> Result<f64, Error>;
}

/// Compute the favor factor of a [`Testcase`]. Lower  is better.
pub trait TestcasePenalty<I, SC> {
    /// Computes the favor factor of a [`Testcase`]. Higher is better.
    fn compute(scheduler: &SC, entry: &Testcase<I>) -> Result<f64, Error>;
}

/// Multiply the testcase size with the execution time.
/// This favors small and quick testcases.
#[derive(Debug, Clone)]
pub struct LenTimeMulTestcasePenalty {}

impl<I, S> TestcasePenalty<I, S> for LenTimeMulTestcasePenalty
where
    I: HasLen,
    S: HasTestcase<I>,
{
    #[expect(clippy::cast_precision_loss)]
    fn compute(state: &S, entry: &Testcase<I>) -> Result<f64, Error> {
        // TODO maybe enforce entry.exec_time().is_some()
        if let Some(testcase_md) = state.testcase_md(entry) {
            Ok(testcase_md.exec_time().map_or(1, |d| d.as_millis()) as f64
                * entry.input_len() as f64)
        } else {
            Ok(1.0)
        }
    }
}
