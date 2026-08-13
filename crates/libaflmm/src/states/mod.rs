//! The fuzzer, and state are the core pieces of every good fuzzer

use crate::monitors::perf_stats::PerfStats;
use crate::{
    Result,
    common::{DependencyResolver, Registrator},
    corpus::{
        InMemoryCorpus, ObjectiveCorpus, ObjectiveInMemoryCorpus, ScheduledCorpus, Scheduler,
        Testcase, TestcaseFilenameFormat, schedulers::NopScheduler, testcase::TestcaseId,
    },
    inputs::{Input, InputContext, NopContext, NopInput},
    launchers::InstanceId,
};
use alloc::{
    borrow::Cow,
    collections::VecDeque,
    string::String,
};
use core::{
    fmt::{self, Debug},
    marker::PhantomData,
    time::Duration,
};
use libaflmm_bolts::{
    NamedSerdeAnyMap, OwnedSlice, SerdeAny, SerdeAnyMap,
    anymap::{named_metadata, named_metadata_mut, unnamed_metadata, unnamed_metadata_mut},
};
use libaflmm_core::runtime;
use num_traits::Zero;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::HashMap, fs::File};
use typed_builder::TypedBuilder;

/// The maximum size of a [`Testcase`]
pub const DEFAULT_MAX_SIZE: usize = 1_048_576;

/// The name used in stats json file for the stability value
pub static STAT_CALIBRATION: Cow<'static, str> = Cow::Borrowed("stability");
/// The name used in stats json file for the coverage value
pub static STAT_COVERAGE: Cow<'static, str> = Cow::Borrowed("coverage");

#[derive(Serialize, Deserialize, Debug, Clone)]
/// The stats the fuzzer produces at intervals.
pub struct Stats {
    pub(crate) pid: InstanceId,
    /// How many times the executor ran the harness/target
    pub(crate) executions: u64,
    /// At what time the fuzzing started
    pub(crate) start_time: Duration,
    /// number of items in [`Corpus`](crate::corpus::Corpus)
    pub(crate) corpus: usize,
    /// number of items in objective [`Corpus`](crate::corpus::Corpus)
    pub(crate) objective: usize,
    /// last time smth was found
    pub(crate) last_found_time: Duration,
    /// hold additional info that users want, in JSON format.
    /// Key is the info name, value is the info in JSON.
    pub(crate) user_map: HashMap<String, String>,
    /// Per-stage performance counters used by the introspection macros.
    pub(crate) perf: PerfStats,
}

/// The state a fuzz run.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(bound = "
        I: serde::Serialize + for<'a> serde::Deserialize<'a>,
        CT: serde::Serialize + for<'a> serde::Deserialize<'a>,
        C: serde::Serialize + for<'a> serde::Deserialize<'a>,
        OC: serde::Serialize + for<'a> serde::Deserialize<'a>,
    ")]
pub struct StdState<C, CT, I, OC, SC> {
    /// the [`InputContext`]. helper to transform [`Input`] into a byte slice
    context: CT,
    /// The [`Corpus`](crate::corpus::Corpus)
    corpus: C,
    // Objectives [`Corpus`](crate::corpus::Corpus)
    objective_corpus: OC,
    /// Metadata stored with names
    named_metadata: NamedSerdeAnyMap,
    /// Metadata stored for each corpus entry
    testcase_metadata: HashMap<TestcaseId, TestcaseMetadata>,
    /// `MaxSize` [`Testcase`] size for [`Mutator`] that appreciate it
    max_size: usize,
    /// Remaining initial [`Input`] to load, if any
    metadata_initialized: bool,
    /// Fuzzing stats
    stats: Stats,
    /// Testcases that remain to be sync'd
    pending_testcases: VecDeque<Testcase<I>>,
    phantom: PhantomData<SC>,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] [{}] execs: {} ({}/s) | corpus: {} | objectives: {}",
            self.pid,
            humantime::format_duration(Duration::from_secs(
                libaflmm_bolts::current_time()
                    .checked_sub(self.start_time)
                    .unwrap()
                    .as_secs()
            )),
            self.executions,
            self.execs_per_sec(),
            self.corpus,
            self.objective,
        )
    }
}

impl Stats {
    /// Update the counter of items in [`Corpus`](crate::corpus::Corpus).
    pub fn update_corpus(&mut self, corpus: usize) {
        self.corpus = corpus;
    }

    /// Update the counter of items in objective [`Corpus`](crate::corpus::Corpus).
    pub fn update_objective(&mut self, objective: usize) {
        self.objective = objective;
    }

    /// Get the exec/sec
    #[must_use]
    pub fn execs_per_sec(&self) -> u64 {
        let as_sec = libaflmm_bolts::current_time()
            .checked_sub(self.start_time)
            .unwrap()
            .as_secs();

        if as_sec.is_zero() {
            0
        } else {
            self.executions / as_sec
        }
    }
}

/// Read the [`Stats`]
pub fn read_stats_json(file: File) -> Result<Stats> {
    serde_json::from_reader(file).map_err(|_| runtime!("Failed to read the stats from a file"))
}

/// Serialize the [`Stats`]
pub fn stats_to_json(stats: &Stats) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(stats).map_err(|_| runtime!("Failed to dump the stats"))
}

/// The trait containing all the stuff that [`StdState`] implements. It's rather a shortcut for typing all the traits
pub trait State: DependencyResolver {
    /// The [`Input`]
    type Input: Input;

    /// The [`Scheduler`]
    type Scheduler: Scheduler;

    /// The associated [`InputContext`]
    type Context: InputContext<Input = Self::Input>;

    /// The associated [`Corpus`](crate::corpus::Corpus)
    type Corpus: ScheduledCorpus<Self::Input, Self::Scheduler>;

    /// The associated objective [`Corpus`](crate::corpus::Corpus)
    type ObjectiveCorpus: ObjectiveCorpus<Self::Input>;

    /// Get the reference to the [`InputContext`]
    fn context(&self) -> &Self::Context;

    /// Get the mutable reference to the [`InputContext`]
    fn context_mut(&mut self) -> &mut Self::Context;

    /// Get the reference to the [`Corpus`](crate::corpus::Corpus)
    fn corpus(&self) -> &Self::Corpus;

    /// Get the mutable reference to the [`Corpus`](crate::corpus::Corpus)
    fn corpus_mut(&mut self) -> &mut Self::Corpus;

    /// Get the reference to the objective [`Corpus`](crate::corpus::Corpus)
    fn objective_corpus(&self) -> &Self::ObjectiveCorpus;

    /// Get the mutable reference to the objective [`Corpus`](crate::corpus::Corpus)
    fn objective_corpus_mut(&mut self) -> &mut Self::ObjectiveCorpus;

    /// Get reference to the [`Testcase`] attached to this [`Testcase`]
    fn testcase_md<'a>(&'a self, tc: &Testcase<Self::Input>) -> Option<&'a TestcaseMetadata>;

    /// Get reference to the [`Testcase`] attached to this [`Testcase`] from [`TestcaseId`]
    fn testcase_md_from_id<'a>(&'a self, id: &TestcaseId) -> Option<&'a TestcaseMetadata>;

    /// Get mutable reference to the [`Testcase`] attached to this [`Testcase`].
    fn testcase_md_mut<'a>(&'a mut self, tc: &Testcase<Self::Input>) -> &'a mut TestcaseMetadata;

    /// Get mutable reference to the [`Testcase`] attached to this [`Testcase`] from [`TestcaseId`]
    fn testcase_md_mut_from_id<'a>(&'a mut self, id: &TestcaseId) -> &'a mut TestcaseMetadata;

    /// Get the [`Testcase`] from [`TestcaseId`]
    fn testcase(&self, id: &TestcaseId) -> Result<Testcase<Self::Input>>;

    /// Get the [`Stats`]
    fn stats(&self) -> &Stats;

    /// Get the [`Stats`] (mutable)
    fn stats_mut(&mut self) -> &mut Stats;

    /// Mutable ref to the introspection [`PerfStats`].
    fn perf_stats_mut(&mut self) -> &mut PerfStats;

    /// The maximum size of an [`Input`]
    fn max_size(&self) -> usize;

    /// The executions counter
    fn executions(&self) -> u64;
    /// Increment the execution counter
    fn increment_execs(&mut self);

    /// The starting time
    fn start_time(&self) -> &Duration;
    /// The starting time (mutable)
    fn start_time_mut(&mut self) -> &mut Duration;

    /// A map, storing all metadata
    fn metadata_map(&self) -> &NamedSerdeAnyMap;
    /// A map, storing all metadata (mutable)
    fn metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap;

    /// should initialize metadata or not?
    fn should_initialize_metadata(&mut self) -> bool;

    /// Does the metadata has the unnamed metadata with type `T`?
    fn has_md<T: SerdeAny>(&self) -> bool {
        self.metadata_map().contains_unnamed::<T>()
    }

    /// Does the metadata has the metadata with type `T` and name `name`?
    fn has_named_md<T: SerdeAny>(&self, name: &str) -> bool {
        self.metadata_map().contains::<T>(name)
    }

    /// Get the reference to the unnamed metadata with type `T`
    fn get_md<T: SerdeAny>(&self) -> Result<&T> {
        unnamed_metadata(self.metadata_map())
    }

    /// Get the mutable reference to the unnamed metadata with type `T`
    fn get_md_mut<T: SerdeAny>(&mut self) -> Result<&mut T> {
        unnamed_metadata_mut(self.metadata_map_mut())
    }

    /// Get the mutable reference to the unnamed metadata with type `T` or insert `value` and return it
    fn get_md_or_insert_with<T: SerdeAny>(&mut self, value: impl FnOnce() -> T) -> &mut T {
        self.metadata_map_mut().get_unnamed_or_insert_with(value)
    }

    /// Get the reference to the metadata with type `T` and name `name`
    fn get_named_md<T: SerdeAny>(&self, name: &str) -> Result<&T> {
        named_metadata(self.metadata_map(), name)
    }

    /// Get the mutable reference to the metadata with type `T` and name `name`
    fn get_named_md_mut<T: SerdeAny>(&mut self, name: &str) -> Result<&mut T> {
        named_metadata_mut(self.metadata_map_mut(), name)
    }

    /// Get the mutable reference to the metadata with type `T` and name `name` or insert `value` and return it
    fn get_named_md_or_insert_with<T: SerdeAny>(
        &mut self,
        name: &str,
        value: impl FnOnce() -> T,
    ) -> &mut T {
        self.metadata_map_mut().get_or_insert_with(name, value)
    }

    fn input_to_bytes<'a>(&mut self, input: &'a Self::Input) -> OwnedSlice<'a, u8> {
        self.context_mut().to_bytes(input)
    }

    /// Ref to the [`Scheduler`]
    fn scheduler(&self) -> &Self::Scheduler {
        self.corpus().scheduler()
    }

    /// Mutable ref to the `Scheduler`
    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        self.corpus_mut().scheduler_mut()
    }

    /// Add testcases to evaluate later on
    fn add_pending_testcases(
        &mut self,
        pending_testcases: impl Iterator<Item = Testcase<Self::Input>>,
    );

    /// Get the next testcase to evaluate
    fn next_pending_testcase(&mut self) -> Option<Testcase<Self::Input>>;
}

/// The [[`Testcase`]] metadata.
#[derive(Serialize, Deserialize, Clone, Debug, Default, TypedBuilder)]
pub struct TestcaseMetadata {
    /// The filename format used to name the [`Testcase`] file on-disk.
    #[builder(default)]
    filename_format: TestcaseFilenameFormat,
    /// Time needed to execute the input
    #[builder(default)]
    exec_time: Option<Duration>,
    /// Number of fuzzing iterations of this particular input updated in `perform_mutational`
    #[builder(default = 0)]
    scheduled_count: usize,
    /// Number of executions done at discovery time
    executions: u64,
    /// If the testcase is "disabled" or not
    #[builder(default = false)]
    disabled: bool,
    /// has found crash (or timeout) or not
    #[builder(default = 0)]
    objectives_found: usize,
    /// A map of metadata, for custom stuff
    map: SerdeAnyMap,
}

impl TestcaseMetadata {
    /// Get the executions
    #[inline]
    #[must_use]
    pub fn executions(&self) -> u64 {
        self.executions
    }

    /// Get the execution time of the [`Testcase`]
    #[inline]
    #[must_use]
    pub fn exec_time(&self) -> &Option<Duration> {
        &self.exec_time
    }

    /// Get the [`Self::scheduled_count`]
    #[inline]
    #[must_use]
    pub fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    /// Get [`Self::disabled`]
    #[inline]
    #[must_use]
    pub fn disabled(&mut self) -> bool {
        self.disabled
    }

    /// Gets how many objectives were found by mutating this testcase
    #[must_use]
    pub fn objectives_found(&self) -> usize {
        self.objectives_found
    }

    /// Get the [`Self::executions`] (mutable)
    #[inline]
    #[must_use]
    pub fn executions_mut(&mut self) -> &mut u64 {
        &mut self.executions
    }

    /// Set the [`Self::executions`]
    #[inline]
    pub fn set_executions(&mut self, executions: u64) {
        self.executions = executions;
    }

    /// Get a mutable reference to the execution time
    #[must_use]
    pub fn exec_time_mut(&mut self) -> &mut Option<Duration> {
        &mut self.exec_time
    }

    /// Sets the execution time of the current testcase
    #[inline]
    pub fn set_exec_time(&mut self, time: Duration) {
        self.exec_time = Some(time);
    }

    /// Set the [`Self::scheduled_count`]
    #[inline]
    pub fn set_scheduled_count(&mut self, scheduled_count: usize) {
        self.scheduled_count = scheduled_count;
    }

    /// Increase the [`Self::increase_scheduled_count`] by 1.
    #[inline]
    pub fn increase_scheduled_count(&mut self) {
        self.scheduled_count += 1;
    }

    /// Set the testcase as disabled
    #[inline]
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Adds one objective to the [`Self::objectives_found`] counter. Mostly called from crash handler or executor.
    pub fn found_objective(&mut self) {
        let count = self.objectives_found.saturating_add(1);
        self.objectives_found = count;
    }

    #[must_use]
    pub fn md_map(&self) -> &SerdeAnyMap {
        &self.map
    }

    pub fn md_map_mut(&mut self) -> &mut SerdeAnyMap {
        &mut self.map
    }
}

impl<C, CT, I, OC, SC> DependencyResolver for StdState<C, CT, I, OC, SC>
where
    C: DependencyResolver + ScheduledCorpus<I, SC>,
    CT: InputContext<Input = I>,
    I: Input,
    OC: DependencyResolver + ObjectiveCorpus<I>,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.corpus.register(registrator)?;
        self.objective_corpus.register(registrator)?;
        Ok(())
    }
}

impl<C, CT, I, OC, SC> State for StdState<C, CT, I, OC, SC>
where
    C: ScheduledCorpus<I, SC>,
    CT: InputContext<Input = I>,
    I: Input,
    OC: ObjectiveCorpus<I>,
    SC: Scheduler,
{
    type Input = I;
    type Scheduler = SC;
    type Context = CT;
    type Corpus = C;
    type ObjectiveCorpus = OC;

    fn context(&self) -> &Self::Context {
        &self.context
    }

    fn context_mut(&mut self) -> &mut Self::Context {
        &mut self.context
    }

    fn objective_corpus(&self) -> &Self::ObjectiveCorpus {
        &self.objective_corpus
    }

    fn objective_corpus_mut(&mut self) -> &mut Self::ObjectiveCorpus {
        &mut self.objective_corpus
    }

    fn corpus(&self) -> &Self::Corpus {
        &self.corpus
    }

    fn corpus_mut(&mut self) -> &mut Self::Corpus {
        &mut self.corpus
    }

    fn testcase(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.corpus.get(id)
    }

    fn testcase_md<'a>(&'a self, tc: &Testcase<I>) -> Option<&'a TestcaseMetadata> {
        self.testcase_metadata.get(tc.id())
    }

    fn testcase_md_from_id<'a>(&'a self, id: &TestcaseId) -> Option<&'a TestcaseMetadata> {
        self.testcase_metadata.get(id)
    }

    fn testcase_md_mut<'a>(&'a mut self, tc: &Testcase<I>) -> &'a mut TestcaseMetadata {
        self.testcase_metadata.entry(*tc.id()).or_default()
    }

    fn testcase_md_mut_from_id<'a>(&'a mut self, id: &TestcaseId) -> &'a mut TestcaseMetadata {
        self.testcase_metadata.entry(*id).or_default()
    }

    fn stats(&self) -> &Stats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut Stats {
        &mut self.stats
    }

    fn perf_stats_mut(&mut self) -> &mut PerfStats {
        &mut self.stats.perf
    }

    /// The max size allowed for this [`Input`]
    fn max_size(&self) -> usize {
        self.max_size
    }

    /// The executions counter
    fn executions(&self) -> u64 {
        self.stats.executions
    }

    /// Increment the execution counter by 1
    fn increment_execs(&mut self) {
        self.stats.executions += 1;
    }

    /// The starting time
    fn start_time(&self) -> &Duration {
        &self.stats.start_time
    }

    /// The starting time (mutable)
    fn start_time_mut(&mut self) -> &mut Duration {
        &mut self.stats.start_time
    }

    /// Get the metadata map from [`NamedSerdeAnyMap`]
    #[inline]
    fn metadata_map(&self) -> &NamedSerdeAnyMap {
        &self.named_metadata
    }

    /// Get the mutable metadata map from [`NamedSerdeAnyMap`]
    #[inline]
    fn metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap {
        &mut self.named_metadata
    }

    fn should_initialize_metadata(&mut self) -> bool {
        if self.metadata_initialized {
            false
        } else {
            self.metadata_initialized = true;
            true
        }
    }

    fn add_pending_testcases(
        &mut self,
        pending_testcases: impl Iterator<Item = Testcase<Self::Input>>,
    ) {
        self.pending_testcases.extend(pending_testcases);
    }

    fn next_pending_testcase(&mut self) -> Option<Testcase<Self::Input>> {
        self.pending_testcases.pop_front()
    }
}

impl<C, CT, I, OC, SC> StdState<C, CT, I, OC, SC>
where
    I: Input,
    C: ScheduledCorpus<I, SC>,
    CT: InputContext<Input = I>,
    OC: ObjectiveCorpus<I>,
{
    /// Creates a new `StdState`, taking ownership of all of the individual components during fuzzing.
    pub fn new(context: CT, corpus: C, objective_corpus: OC) -> Result<Self>
    where
        OC: Serialize + DeserializeOwned,
        C: Serialize + DeserializeOwned,
    {
        let state = Self {
            context,
            stats: Stats {
                pid: std::process::id(),
                executions: 0,
                corpus: 0,
                objective: 0,
                last_found_time: libaflmm_bolts::current_time(),
                start_time: libaflmm_bolts::current_time(),
                user_map: HashMap::new(),
                perf: PerfStats::new(),
            },
            named_metadata: NamedSerdeAnyMap::default(),
            corpus,
            objective_corpus,
            max_size: DEFAULT_MAX_SIZE,
            testcase_metadata: HashMap::new(),
            metadata_initialized: false,
            pending_testcases: VecDeque::new(),
            phantom: PhantomData,
        };
        Ok(state)
    }
}

/// A very simple [`State`] with minimal capabilities, for testing.
///
/// It is a [`StdState`] backed by in-memory corpora and a [`NopContext`].
/// Build one with [`StdState::nop`].
pub type NopState = StdState<
    InMemoryCorpus<NopInput, NopScheduler>,
    NopContext,
    NopInput,
    ObjectiveInMemoryCorpus<NopInput>,
    NopScheduler,
>;

impl NopState {
    /// Create an empty [`StdState`] that has very minimal uses.
    /// Potentially good for testing.
    pub fn nop() -> Result<Self> {
        StdState::new(
            NopContext,
            InMemoryCorpus::nop(),
            ObjectiveInMemoryCorpus::new(),
        )
    }
}

#[cfg(test)]
mod test {
    use crate::states::StdState;

    #[test]
    fn test_std_state() {
        StdState::nop().expect("couldn't instantiate the test state");
    }
}
