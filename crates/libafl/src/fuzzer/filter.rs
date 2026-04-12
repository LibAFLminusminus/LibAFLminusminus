/// A trait to determine if a input should be run or not
pub trait InputFilter<EM, I, S> {
    /// should run execution for this input or no
    fn should_execute(&mut self, input: &I, state: &mut S) -> Result<bool, Error>;
}

/// A pseudo-filter that will execute each input.
#[derive(Debug, Copy, Clone)]
pub struct NopInputFilter;
impl<EM, I, S> InputFilter<EM, I, S> for NopInputFilter {
    #[inline]
    fn should_execute(&mut self, _input: &I, _state: &mut S) -> Result<bool, Error> {
        Ok(true)
    }
}

/// A filter that probabilistically prevents duplicate execution of the same input based on a bloom filter.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct BloomInputFilter {
    bloom: BloomFilter,
}

#[cfg(feature = "std")]
impl Default for BloomInputFilter {
    fn default() -> Self {
        let bloom = BloomFilter::with_false_pos(1e-4).expected_items(10_000_000);
        Self { bloom }
    }
}

#[cfg(feature = "std")]
impl BloomInputFilter {
    #[must_use]
    /// Constructor
    pub fn new(items_count: usize, fp_p: f64) -> Self {
        let bloom = BloomFilter::with_false_pos(fp_p).expected_items(items_count);
        Self { bloom }
    }
}

#[cfg(feature = "std")]
impl<EM, I: Hash, S> InputFilter<I, S> for BloomInputFilter {
    #[inline]
    fn should_execute(&mut self, input: &I, _state: &mut S) -> Result<bool, Error> {
        Ok(!self.bloom.insert(input))
    }
}

/// Wrapper for input filters that report the ratios of skipped to executed inputs.
///
/// The total execution count may be slightly different from what is reported by anything relying
/// on the execution count in the state, because this wrapper only counts executions that are
/// triggered by [`Evaluator::evaluate_filtered`]. Some parts of ``LibAFL`` may use lower-level calls,
/// which are not counted by this wrapper. Notable examples are [`crate::stages::CalibrationStage`]
/// and [`crate::state::StdState::generate_initial_inputs`].
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct ReportingInputFilter<F> {
    inner: F,
    reporting_interval: u64,
}

#[cfg(feature = "std")]
impl<F> ReportingInputFilter<F> {
    /// Create a new [`ReportingInputFilter`] around an existing input filter. It will report the ratio of skipped to executed inputs every `reporting_interval` executions.
    pub fn new(inner: F, reporting_interval: u64) -> Self {
        Self {
            inner,
            reporting_interval,
        }
    }
}

#[cfg(feature = "std")]
impl_serdeany!(ReportingInputFilterStats);

#[cfg(feature = "std")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ReportingInputFilterStats {
    skipped: u64,
}

#[cfg(feature = "std")]
impl<F, I, S> InputFilter<I, S> for ReportingInputFilter<F>
where
    F: InputFilter<EM, I, S>,
    S: HasMetadata + HasExecutions,
{
    fn should_execute(&mut self, input: &I, state: &mut S) -> Result<bool, Error> {
        let actual_executions = *state.executions();
        let should_execute = self.inner.should_execute(input, state, manager)?;

        let stats = state.metadata_or_insert_with(ReportingInputFilterStats::default);

        if !should_execute {
            stats.skipped += 1;
        }

        let skipped = stats.skipped;
        let attempted_executions = skipped + actual_executions;

        Ok(should_execute)
    }
}
