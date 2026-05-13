//! The fuzzer, and state are the core pieces of every good fuzzer

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{
    any::type_name,
    fmt::{self, Debug},
    marker::PhantomData,
    time::Duration,
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
};

use libafl_bolts::{
    rands::{Rand, StdRand},
    serdeany::{NamedSerdeAnyMap, SerdeAny, SerdeAnyMap},
};
#[cfg(not(debug_assertions))]
use libafl_core::nonzero_macros::non_zero_unchecked;
use nix::fcntl::{Flock, FlockArg};
use num_traits::Zero;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use typed_builder::TypedBuilder;

use crate::{
    Error, Result,
    corpus::{
        Corpus, InMemoryCorpus, Scheduler, Testcase, TestcaseFilenameFormat,
        schedulers::NopScheduler, testcase::TestcaseId,
    },
    dependency::{DependencyResolver, Registrator},
    fuzzers::{EvaluationResult, Evaluator},
    generators::Generator,
    inputs::{Input, InputContext, NopContext, NopInput},
    launchers::InstanceId,
    runtimes::RuntimeHandle,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// The stats the fuzzer produces at intervals.
pub struct Stats {
    pub(crate) pid: InstanceId,
    /// How many times the executor ran the harness/target
    pub(crate) executions: u64,
    /// At what time the fuzzing started
    pub(crate) start_time: Duration,
    /// number of items in [`Corpus`]
    pub(crate) corpus: usize,
    /// number of items in objective [`Corpus`]
    pub(crate) objective: usize,
    /// last time smth was found
    pub(crate) last_found_time: Duration,
    /// [`NamedSerdeAnyMap`] to hold additional info that users want
    pub(crate) user_map: NamedSerdeAnyMap,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] [{}] execs: {} ({}/s) | corpus: {} | objectives: {}",
            self.pid,
            humantime::format_duration(Duration::from_secs(
                libafl_bolts::current_time()
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
    /// Update the counter of items in [`Corpus`].
    pub fn update_corpus(&mut self, corpus: usize) {
        self.corpus = corpus;
    }

    /// Update the counter of items in objective [`Corpus`].
    pub fn update_objective(&mut self, objective: usize) {
        self.objective = objective;
    }

    /// Get the exec/sec
    #[must_use]
    pub fn execs_per_sec(&self) -> u64 {
        let as_sec = libafl_bolts::current_time()
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
    let mut locked =
        Flock::lock(file, FlockArg::LockShared).map_err(|(_, e)| nix::Error::from(e))?;
    locked.seek(SeekFrom::Start(0))?;
    serde_json::from_reader(&mut *locked)
        .map_err(|_| Error::runtime("Failed to read the stats from a file"))
}

/// Put the [`Stats`] into the file
pub fn sync_stats(file: File, stats: &Stats) -> Result<()> {
    let mut locked =
        Flock::lock(file, FlockArg::LockExclusive).map_err(|(_, e)| nix::Error::from(e))?;
    locked.set_len(0)?;
    locked.seek(SeekFrom::Start(0))?;
    serde_json::to_writer_pretty(&mut *locked, stats)
        .map_err(|_| Error::runtime("Failed to dump the stats to a file"))
}

/// the all-in-one trait for all the data that normal state should contain
pub trait FlatState {
    /// Get the [`Stats`]
    fn stats(&self) -> &Stats;

    /// Get the [`Stats`] (mutable)
    fn stats_mut(&mut self) -> &mut Stats;
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
}

/// The trait containing all the stuff that [`StdState`] implements. It's rather a shortcut for typing all the traits
pub trait State<I>:
    FlatState
    + DependencyResolver
    + HasCorpus<I>
    + HasObjectiveCorpus<I>
    + HasScheduler
    + HasTestcase<I>
    + HasContext<I>
{
}

/// This module has a [`Scheduler`]
pub trait HasScheduler {
    /// [`Scheduler`] type
    type Scheduler: Scheduler;

    /// Ref to the [`Scheduler`]
    fn scheduler(&self) -> &Self::Scheduler;
    /// Mutable ref to the `Scheduler`
    fn scheduler_mut(&mut self) -> &mut Self::Scheduler;
}

/// This module has a [`Testcase`]
pub trait HasTestcase<I> {
    /// Get reference to the [`Testcase`] attached to this [`Testcase`]
    fn testcase_md<'a>(&'a self, tc: &Testcase<I>) -> Option<&'a TestcaseMetadata>;
    /// Get reference to the [`Testcase`] attached to this [`Testcase`] from [`TestcaseId`]
    fn testcase_md_from_id<'a>(&'a self, id: &TestcaseId) -> Option<&'a TestcaseMetadata>;
    /// Get mutable reference to the [`Testcase`] attached to this [`Testcase`].
    fn testcase_md_mut<'a>(&'a mut self, tc: &Testcase<I>) -> &'a mut TestcaseMetadata;
    /// Get mutable reference to the [`Testcase`] attached to this [`Testcase`] from [`TestcaseId`]
    fn testcase_md_mut_from_id<'a>(&'a mut self, id: &TestcaseId) -> &'a mut TestcaseMetadata;
    /// Get the [`Testcase`] from [`TestcaseId`]
    fn testcase(&self, id: &TestcaseId) -> Result<Testcase<I>>;
}

impl<C, CT, I, OC, SC> HasTestcase<I> for StdState<C, CT, I, OC, SC>
where
    C: Corpus<I>,
{
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
}

/// This module has a [`Corpus`]
pub trait HasCorpus<I> {
    /// The associated [`Corpus`]
    type Corpus: Corpus<I>;

    /// Get the reference to the [`Corpus`]
    fn corpus(&self) -> &Self::Corpus;
    /// Get the mutable reference to the [`Corpus`]
    fn corpus_mut(&mut self) -> &mut Self::Corpus;
}

impl<C, CT, I, OC, SC> HasCorpus<I> for StdState<C, CT, I, OC, SC>
where
    C: Corpus<I>,
{
    type Corpus = C;

    fn corpus(&self) -> &Self::Corpus {
        &self.corpus
    }

    fn corpus_mut(&mut self) -> &mut Self::Corpus {
        &mut self.corpus
    }
}

/// This module has a `Corpus` for objectives
pub trait HasObjectiveCorpus<I> {
    /// The associated objective [`Corpus`]
    type Corpus: Corpus<I>;

    /// Get the reference to the objective [`Corpus`]
    fn objective_corpus(&self) -> &Self::Corpus;
    /// Get the mutable reference to the objective [`Corpus`]
    fn objective_corpus_mut(&mut self) -> &mut Self::Corpus;
}

impl<C, CT, I, OC, SC> HasObjectiveCorpus<I> for StdState<C, CT, I, OC, SC>
where
    OC: Corpus<I>,
{
    type Corpus = OC;

    fn objective_corpus(&self) -> &Self::Corpus {
        &self.objective_corpus
    }

    fn objective_corpus_mut(&mut self) -> &mut Self::Corpus {
        &mut self.objective_corpus
    }
}

/// This module has a `InputContext`
pub trait HasContext<I> {
    /// The associated [`InputContext`]
    type Context: InputContext<I>;

    /// Get the mutable reference to the [`InputContext`]
    fn context_mut(&mut self) -> &mut Self::Context;
}

impl<C, CT, I, OC, SC> HasContext<I> for StdState<C, CT, I, OC, SC>
where
    CT: InputContext<I>,
{
    type Context = CT;

    fn context_mut(&mut self) -> &mut Self::Context {
        &mut self.context
    }
}

impl<C, CT, I, OC, SC> HasScheduler for StdState<C, CT, I, OC, SC>
where
    C: Corpus<I>,
{
    type Scheduler = C::Scheduler;

    fn scheduler(&self) -> &Self::Scheduler {
        self.corpus.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        self.corpus.scheduler_mut()
    }
}

/// The maximum size of a [`Testcase`]
pub const DEFAULT_MAX_SIZE: usize = 1_048_576;

/// Struct that holds the options for input loading
pub struct LoadConfig<'a, I, S, Z> {
    /// Function to load input from a Path
    loader: &'a mut dyn FnMut(&mut Z, &mut S, &Path) -> Result<I>,
    /// Error if Input leads to a Solution.
    exit_on_solution: bool,
}

impl<I, S, Z> Debug for LoadConfig<'_, I, S, Z> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LoadConfig {{}}")
    }
}

/// The state a fuzz run.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(bound = "
        CT: serde::Serialize + for<'a> serde::Deserialize<'a>,
        C: serde::Serialize + for<'a> serde::Deserialize<'a>,
        OC: serde::Serialize + for<'a> serde::Deserialize<'a>,
        SC: serde::Serialize + for<'a> serde::Deserialize<'a>,
    ")]
pub struct StdState<C, CT, I, OC, SC> {
    /// the [`InputContext`]. helper to transform [`Input`] into a byte slice
    context: CT,
    /// The [`Corpus`]
    corpus: C,
    // Objectives [`Corpus`]
    objective_corpus: OC,
    /// Metadata stored with names
    named_metadata: NamedSerdeAnyMap,
    /// Metadata stored for each corpus entry
    testcase_metadata: HashMap<TestcaseId, TestcaseMetadata>,
    /// `MaxSize` [`Testcase`] size for [`Mutator`] that appreciate it
    max_size: usize,
    /// Remaining initial [`Input`] to load, if any
    remaining_initial_files: Option<Vec<PathBuf>>,
    /// symlinks we have already traversed when loading [`Self::remaining_initial_files`]
    dont_reenter: Option<Vec<PathBuf>>,
    metadata_initialized: bool,
    stats: Stats,
    phantom: PhantomData<(I, SC)>,
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
}

/// Add a named metadata to the metadata map
#[inline]
pub fn add_named_metadata<M>(map: &mut NamedSerdeAnyMap, name: &str, meta: M)
where
    M: SerdeAny,
{
    map.insert(name, meta);
}

/// To get named metadata from a [`NamedSerdeAnyMap`]
#[inline]
pub fn named_metadata<'a, M>(map: &'a NamedSerdeAnyMap, name: &str) -> Result<&'a M>
where
    M: SerdeAny,
{
    map.get::<M>(name)
        .ok_or_else(|| Error::key_not_found(format!("{} not found", type_name::<M>())))
}

/// To get an unnamed metadata from a [`NamedSerdeAnyMap`]
#[inline]
pub fn unnamed_metadata<M>(map: &NamedSerdeAnyMap) -> Result<&M>
where
    M: SerdeAny,
{
    map.get::<M>("")
        .ok_or_else(|| Error::key_not_found(format!("{} not found", type_name::<M>())))
}

/// To get mutable named metadata from a [`NamedSerdeAnyMap`]
#[inline]
pub fn named_metadata_mut<'a, M>(map: &'a mut NamedSerdeAnyMap, name: &str) -> Result<&'a mut M>
where
    M: SerdeAny,
{
    map.get_mut::<M>(name)
        .ok_or_else(|| Error::key_not_found(format!("{} not found", type_name::<M>())))
}

/// To get mutable unnamed metadata from a [`NamedSerdeAnyMap`]
#[inline]
pub fn unnamed_metadata_mut<M>(map: &mut NamedSerdeAnyMap) -> Result<&mut M>
where
    M: SerdeAny,
{
    map.get_mut::<M>("")
        .ok_or_else(|| Error::key_not_found(format!("{} not found", type_name::<M>())))
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

    /// Get the filename
    #[must_use]
    pub fn get_filename(&self, id: &str) -> String {
        match &self.filename_format {
            TestcaseFilenameFormat::Id => id.to_string(),
            TestcaseFilenameFormat::Prefix(prefix) => format!("{prefix}-{id}"),
            TestcaseFilenameFormat::Custom(custom_name) => custom_name.clone(),
        }
    }

    /// Set the filename of this [`Testcase`]
    pub fn set_filename(&mut self, filename: TestcaseFilenameFormat) {
        self.filename_format = filename;
    }
}

/// The metadata for each testcase used in power schedules.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub struct PSMetadata {
    /// Number of bits set in bitmap.
    bitmap_size: u64,
    /// Number of queue cycles behind
    handicap: u64,
    /// Path depth, initialized in `on_add`
    depth: u64,
    /// Cycles used to calibrate this.
    cycle_and_time: (Duration, usize),
}

impl PSMetadata {
    /// Create new [`struct@PSMetadata`]
    #[must_use]
    pub fn new(depth: u64) -> Self {
        Self {
            bitmap_size: 0,
            handicap: 0,
            depth,
            cycle_and_time: (Duration::default(), 0),
        }
    }

    /// Get the [`Self::bitmap_size`]
    #[inline]
    #[must_use]
    pub fn bitmap_size(&self) -> u64 {
        self.bitmap_size
    }

    /// Set the [`Self::bitmap_size`]
    #[inline]
    pub fn set_bitmap_size(&mut self, val: u64) {
        self.bitmap_size = val;
    }

    /// Get the [`Self::handicap`]
    #[inline]
    #[must_use]
    pub fn handicap(&self) -> u64 {
        self.handicap
    }

    /// Set the [`Self::handicap`]
    #[inline]
    pub fn set_handicap(&mut self, val: u64) {
        self.handicap = val;
    }

    /// Get the [`Self::depth`]
    #[inline]
    #[must_use]
    pub fn depth(&self) -> u64 {
        self.depth
    }

    /// Set the [`Self::depth`]
    #[inline]
    pub fn set_depth(&mut self, val: u64) {
        self.depth = val;
    }

    /// Get the [`Self::cycle_and_time`]
    #[inline]
    #[must_use]
    pub fn cycle_and_time(&self) -> (Duration, usize) {
        self.cycle_and_time
    }

    #[inline]
    /// Set the [`Self::cycle_and_time`]
    pub fn set_cycle_and_time(&mut self, cycle_and_time: (Duration, usize)) {
        self.cycle_and_time = cycle_and_time;
    }
}

libafl_bolts::impl_serdeany!(PSMetadata);

impl<C, CT, I, OC, SC> FlatState for StdState<C, CT, I, OC, SC> {
    fn stats(&self) -> &Stats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut Stats {
        &mut self.stats
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
}

impl<C, CT, I, OC, SC> DependencyResolver for StdState<C, CT, I, OC, SC>
where
    C: DependencyResolver + Corpus<I>,
    OC: DependencyResolver + Corpus<I>,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        self.corpus_mut().register(registrator)?;
        self.objective_corpus_mut().register(registrator)?;

        Ok(())
    }
}

impl<C, CT, I, OC, SC> State<I> for StdState<C, CT, I, OC, SC>
where
    C: Corpus<I>,
    CT: InputContext<I>,
    OC: Corpus<I>,
{
}

impl<C, CT, I, OC, SC> StdState<C, CT, I, OC, SC>
where
    C: Corpus<I>,
    I: Input,
    OC: Corpus<I>,
{
    /// Decide if the state must load the inputs
    pub fn must_load_initial_inputs(&self) -> bool {
        self.corpus().count() == 0
            || (self.remaining_initial_files.is_some()
                && !self.remaining_initial_files.as_ref().unwrap().is_empty())
    }

    /// List initial inputs from a directory.
    fn next_file(&mut self) -> Result<PathBuf> {
        loop {
            if let Some(path) = self.remaining_initial_files.as_mut().and_then(Vec::pop) {
                let filename = path.file_name().unwrap().to_string_lossy();
                if filename.starts_with('.')
                // || filename
                //     .rsplit_once('-')
                //     .is_some_and(|(_, s)| u64::from_str(s).is_ok())
                {
                    continue;
                }

                let attributes = fs::metadata(&path);

                if attributes.is_err() {
                    continue;
                }

                let attr = attributes?;

                if attr.is_file() && attr.len() > 0 {
                    return Ok(path);
                } else if attr.is_dir() {
                    let files = self.remaining_initial_files.as_mut().unwrap();
                    path.read_dir()?
                        .try_for_each(|entry| entry.map(|e| files.push(e.path())))?;
                } else if attr.is_symlink() {
                    let path = fs::canonicalize(path)?;
                    let dont_reenter = self.dont_reenter.get_or_insert_with(Default::default);
                    if dont_reenter.iter().any(|p| path.starts_with(p)) {
                        continue;
                    }
                    if path.is_dir() {
                        dont_reenter.push(path.clone());
                    }
                    let files = self.remaining_initial_files.as_mut().unwrap();
                    files.push(path);
                }
            } else {
                return Err(Error::iterator_end("No remaining files to load."));
            }
        }
    }

    /// Resets the state of initial files.
    fn reset_initial_files_state(&mut self) {
        self.remaining_initial_files = None;
        self.dont_reenter = None;
    }

    /// Sets canonical paths for provided inputs
    fn canonicalize_input_dirs<P>(&mut self, in_dirs: &[P]) -> Result<()>
    where
        P: AsRef<Path>,
    {
        if let Some(remaining) = self.remaining_initial_files.as_ref() {
            // everything was loaded
            if remaining.is_empty() {
                return Ok(());
            }
        } else {
            let files = in_dirs.iter().try_fold(Vec::new(), |mut res, file| {
                file.as_ref().canonicalize().map(|canonicalized| {
                    res.push(canonicalized);
                    res
                })
            })?;
            self.dont_reenter = Some(files.clone());
            self.remaining_initial_files = Some(files);
        }
        Ok(())
    }

    /// Loads initial inputs from the passed-in `in_dirs`.
    /// If `forced` is true, will add all testcases, no matter what.
    /// This method takes a list of files.
    fn load_initial_inputs_custom_by_filenames<E, P, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        file_list: &[P],
        load_config: LoadConfig<I, Self, Z>,
    ) -> Result<()>
    where
        P: AsRef<Path>,
        Z: Evaluator<E, I, Self, W>,
    {
        if let Some(remaining) = self.remaining_initial_files.as_ref() {
            // everything was loaded
            if remaining.is_empty() {
                return Ok(());
            }
        } else {
            self.remaining_initial_files =
                Some(file_list.iter().map(|p| p.as_ref().to_path_buf()).collect());
        }

        self.continue_loading_initial_inputs_custom(fuzzer, executor, rt_handle, load_config)
    }

    fn load_file<E, W, Z>(
        &mut self,
        path: &Path,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        config: &mut LoadConfig<I, Self, Z>,
    ) -> Result<EvaluationResult>
    where
        Z: Evaluator<E, I, Self, W>,
    {
        log::info!("Loading file {} ...", path.display());
        let input = match (config.loader)(fuzzer, self, path) {
            Ok(input) => input,
            Err(err) => {
                log::error!(
                    "Skipping input that we could not load from {}: {err:?}",
                    path.display()
                );
                return Ok(EvaluationResult::not_interesting());
            }
        };
        let res = fuzzer.evaluate_input(self, executor, rt_handle, &input)?;
        Ok(res)
    }

    /// Loads initial inputs from the passed-in `in_dirs`.
    /// This method takes a list of files and a `LoadConfig`
    /// which specifies the special handling of initial inputs
    fn continue_loading_initial_inputs_custom<E, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        mut config: LoadConfig<I, Self, Z>,
    ) -> Result<()>
    where
        Z: Evaluator<E, I, Self, W>,
    {
        loop {
            match self.next_file() {
                Ok(path) => {
                    let res = self.load_file(&path, fuzzer, executor, rt_handle, &mut config)?;
                    if config.exit_on_solution && res.is_objective_worthy() {
                        return Err(Error::invalid_corpus(format!(
                            "Input {} resulted in a objective.",
                            path.display()
                        )));
                    }
                }
                Err(Error::IteratorEnd(_, _)) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// Recursively walk supplied corpus directories
    pub fn walk_initial_inputs<F, P>(&mut self, in_dirs: &[P], mut closure: F) -> Result<()>
    where
        F: FnMut(&Path) -> Result<()>,
        P: AsRef<Path>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        loop {
            match self.next_file() {
                Ok(path) => {
                    closure(&path)?;
                }
                Err(Error::IteratorEnd(_, _)) => break,
                Err(e) => return Err(e),
            }
        }
        self.reset_initial_files_state();
        Ok(())
    }

    /// Loads all intial inputs, even if they are not considered `interesting`.
    /// This is rarely the right method, use `load_initial_inputs`,
    /// and potentially fix your `Feedback`, instead.
    /// This method takes a list of files, instead of folders.
    pub fn load_initial_inputs_by_filenames<E, P, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        file_list: &[P],
    ) -> Result<()>
    where
        P: AsRef<Path>,
        Z: Evaluator<E, I, Self, W>,
    {
        self.load_initial_inputs_custom_by_filenames(
            fuzzer,
            executor,
            rt_handle,
            file_list,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                exit_on_solution: false,
            },
        )
    }

    /// Loads all intial inputs, even if they are not considered `interesting`.
    /// This is rarely the right method, use `load_initial_inputs`,
    /// and potentially fix your `Feedback`, instead.
    pub fn load_initial_inputs_forced<E, P, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        in_dirs: &[P],
    ) -> Result<()>
    where
        P: AsRef<Path>,
        Z: Evaluator<E, I, Self, W>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        self.continue_loading_initial_inputs_custom(
            fuzzer,
            executor,
            rt_handle,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                exit_on_solution: false,
            },
        )
    }
    /// Loads initial inputs from the passed-in `in_dirs`.
    /// If `forced` is true, will add all testcases, no matter what.
    /// This method takes a list of files, instead of folders.
    pub fn load_initial_inputs_by_filenames_forced<E, P, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        file_list: &[P],
    ) -> Result<()>
    where
        P: AsRef<Path>,
        Z: Evaluator<E, I, Self, W>,
    {
        self.load_initial_inputs_custom_by_filenames(
            fuzzer,
            executor,
            rt_handle,
            file_list,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                exit_on_solution: false,
            },
        )
    }

    /// Loads initial inputs from the passed-in `in_dirs`.
    pub fn load_initial_inputs<E, P, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        in_dirs: &[P],
    ) -> Result<()>
    where
        P: AsRef<Path>,
        Z: Evaluator<E, I, Self, W>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        self.continue_loading_initial_inputs_custom(
            fuzzer,
            executor,
            rt_handle,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                exit_on_solution: false,
            },
        )
    }

    /// Loads initial inputs from the passed-in `in_dirs`.
    /// Will return a `CorpusError` if a solution is found
    pub fn load_initial_inputs_disallow_solution<E, P, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<Self, W>,
        in_dirs: &[P],
    ) -> Result<()>
    where
        P: AsRef<Path>,
        Z: Evaluator<E, I, Self, W>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        self.continue_loading_initial_inputs_custom(
            fuzzer,
            executor,
            rt_handle,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                exit_on_solution: true,
            },
        )
    }
}

impl<C, CT, I, OC, SC> StdState<C, CT, I, OC, SC> {
    /// Generate `num` initial inputs, using the passed-in generator.
    pub fn generate_initial_inputs<G, E, R, W, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        generator: &mut G,
        rand: &mut R,
        rt_handle: &mut RuntimeHandle<Self, W>,
        num: usize,
    ) -> Result<usize>
    where
        R: Rand,
        G: Generator<I, R, Self>,
        Z: Evaluator<E, I, Self, W>,
    {
        let mut added = 0;

        for _ in 0..num {
            let input = generator.generate(rand, self)?;
            let res = fuzzer.evaluate_input(self, executor, rt_handle, &input)?;
            if res.is_corpus_worthy() {
                added += 1;
            }
        }

        Ok(added)
    }
}

impl<C, CT, I, OC, SC> StdState<C, CT, I, OC, SC>
where
    C: Corpus<I, Scheduler = SC>,
    I: Input,
    OC: Corpus<I>,
{
    /// Creates a new `StdState`, taking ownership of all of the individual components during fuzzing.
    pub fn new(context: CT, corpus: C, objective_corpus: OC) -> Result<Self>
    where
        OC: Serialize + DeserializeOwned + DependencyResolver,
        C: Serialize + DeserializeOwned + DependencyResolver,
    {
        let state = Self {
            context,
            stats: Stats {
                pid: std::process::id(),
                executions: 0,
                corpus: 0,
                objective: 0,
                last_found_time: libafl_bolts::current_time(),
                start_time: libafl_bolts::current_time(),
                user_map: NamedSerdeAnyMap::new(),
            },
            named_metadata: NamedSerdeAnyMap::default(),
            corpus,
            objective_corpus,
            max_size: DEFAULT_MAX_SIZE,
            remaining_initial_files: None,
            dont_reenter: None,
            phantom: PhantomData,
            testcase_metadata: HashMap::new(),
            metadata_initialized: false,
        };
        Ok(state)
    }
}

impl
    StdState<
        InMemoryCorpus<NopInput, NopScheduler>,
        NopContext,
        NopInput,
        InMemoryCorpus<NopInput, NopScheduler>,
        NopScheduler,
    >
{
    /// Create an empty [`StdState`] that has very minimal uses.
    /// Potentially good for testing.
    pub fn nop() -> Result<Self> {
        StdState::new(
            NopContext,
            InMemoryCorpus::<NopInput, NopScheduler>::new(NopScheduler),
            InMemoryCorpus::new(NopScheduler),
        )
    }
}

/// A very simple state without any bells or whistles, for testing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NopState<I> {
    metadata: SerdeAnyMap,
    named_metadata: NamedSerdeAnyMap,
    execution: u64,
    stop_requested: bool,
    rand: StdRand,
    phantom: PhantomData<I>,
}

impl<I> NopState<I> {
    /// Create a new [`NopState`] that does nothing (for testing purposes usually)
    #[must_use]
    pub fn new() -> Self {
        NopState {
            metadata: SerdeAnyMap::new(),
            named_metadata: NamedSerdeAnyMap::new(),
            execution: 0,
            rand: StdRand::default(),
            stop_requested: false,
            phantom: PhantomData,
        }
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
