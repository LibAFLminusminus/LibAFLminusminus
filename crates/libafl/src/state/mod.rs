//! The fuzzer, and state are the core pieces of every good fuzzer

use alloc::string::String;
#[cfg(feature = "std")]
use alloc::vec::Vec;
use core::{borrow::BorrowMut, fmt::Debug, marker::PhantomData, time::Duration};
#[cfg(feature = "std")]
use std::{
    fs,
    path::{Path, PathBuf},
};
use typed_builder::TypedBuilder;

#[cfg(feature = "std")]
use libafl_bolts::core_affinity::{CoreId, Cores};
use libafl_bolts::{
    rands::{Rand, StdRand},
    serdeany::{NamedSerdeAnyMap, SerdeAnyMap},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[cfg(feature = "introspection")]
use crate::monitors::stats::ClientPerfStats;
use crate::{
    Error, HasMetadata, HasNamedMetadata,
    corpus::{
        Corpus, CorpusId, InMemoryCorpus, Testcase, TestcaseFilenameFormat,
        schedulers::NopScheduler,
    },
    feedbacks::StateInitializer,
    fuzzer::{Evaluator, ExecuteInputResult},
    generators::Generator,
    inputs::{Input, NopInput},
};

pub trait State<C, I, R, SC, SO> {
    fn max_size(&self) -> usize;
    /// Set the max size.
    fn max_size_mut(&mut self);

    /// The executions counter
    fn executions(&self);

    /// The executions counter (mutable)
    fn executions_mut(&mut self);
    ///the imported testcases counter
    fn imported(&self) -> usize;
    ///the imported testcases counter (mutable)
    fn imported_mut(&mut self);
    /// The starting time
    fn start_time(&self) -> &Duration;
    /// The starting time (mutable)
    fn start_time_mut(&mut self) -> &mut Duration;
    /// The last time we found something by ourselves
    fn last_found_time(&self) -> &Duration;
    /// The last time we found something by ourselves (mutable)
    fn last_found_time_mut(&mut self) -> &mut Duration;
    /// The last time we reported progress,if available/used.
    /// This information is used by fuzzer `maybe_report_progress`.
    fn last_report_time(&self) -> &Option<Duration>;
    /// The last time we reported progress,if available/used (mutable).
    /// This information is used by fuzzer `maybe_report_progress`.
    fn last_report_time_mut(&mut self) -> &mut Option<Duration>;
    /// The solutions corpus
    fn solutions(&self) -> &SO;
    /// The solutions corpus (mutable)
    fn solutions_mut(&mut self) -> &mut SO;
    /// Returns the corpus
    #[inline]
    fn corpus(&self) -> &C;
    /// Returns the mutable corpus
    #[inline]
    fn corpus_mut(&mut self) -> &mut C;
    /// The rand instance
    #[inline]
    fn rand(&self) -> &R;
    /// The rand instance (mutable)
    #[inline]
    fn rand_mut(&mut self) -> &mut R;
    /// To get the testcase
    fn testcase(&self, id: CorpusId) -> &Testcase<I>;
    /// To get mutable testcase
    fn testcase_mut(&self, id: CorpusId) -> &mut Testcase<I>;
    /// Get all the metadata into an [`hashbrown::HashMap`]
    #[inline]
    fn named_metadata_map(&self) -> &NamedSerdeAnyMap;
    /// Get all the metadata into an [`hashbrown::HashMap`] (mutable)
    #[inline]
    fn named_metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap;
    fn request_stop(&mut self);

    fn discard_stop_request(&mut self);

    fn stop_requested(&self) -> bool;

    fn set_corpus_id(&mut self, id: CorpusId) -> Result<(), Error>;

    fn current_corpus_id(&self) -> Result<Option<CorpusId>, Error>;

    // to romain: are these 3 methods safely handling of the corpus?

    fn current_testcase(&self) -> Result<&Testcase<I>, Error>;

    fn current_testcase_mut(&self) -> Result<&mut Testcase<I>, Error>;

    #[cfg(feature = "introspection")]
    fn introspection_stats(&self) -> &ClientPerfStats;

    #[cfg(feature = "introspection")]
    fn introspection_stats_mut(&mut self) -> &mut ClientPerfStats;
}

/// The maximum size of a testcase
pub const DEFAULT_MAX_SIZE: usize = 1_048_576;

/// Struct that holds the options for input loading
#[cfg(feature = "std")]
pub struct LoadConfig<'a, I, S, Z> {
    /// Load Input even if it was deemed "uninteresting" by the fuzzer
    forced: bool,
    /// Function to load input from a Path
    loader: &'a mut dyn FnMut(&mut Z, &mut S, &Path) -> Result<I, Error>,
    /// Error if Input leads to a Solution.
    exit_on_solution: bool,
}

#[cfg(feature = "std")]
impl<I, S, Z> Debug for LoadConfig<'_, I, S, Z> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "LoadConfig {{}}")
    }
}

/// The state a fuzz run.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(bound = "
        C: serde::Serialize + for<'a> serde::Deserialize<'a>,
        R: serde::Serialize + for<'a> serde::Deserialize<'a>,
        SC: serde::Serialize + for<'a> serde::Deserialize<'a>,
    ")]
pub struct StdState<C, I, R, SC, SO> {
    /// RNG instance
    rand: R,
    /// How many times the executor ran the harness/target
    executions: u64,
    /// At what time the fuzzing started
    start_time: Duration,
    /// the number of new paths that imported from other fuzzers
    imported: usize,
    /// The corpus
    corpus: C,
    // Solutions corpus
    solutions: SO,
    /// Metadata stored with names
    named_metadata: NamedSerdeAnyMap,
    /// `MaxSize` testcase size for mutators that appreciate it
    max_size: usize,
    /// Performance statistics for this fuzzer
    #[cfg(feature = "introspection")]
    introspection_stats: ClientPerfStats,
    #[cfg(feature = "std")]
    /// Remaining initial inputs to load, if any
    remaining_initial_files: Option<Vec<PathBuf>>,
    #[cfg(feature = "std")]
    /// symlinks we have already traversed when loading `remaining_initial_files`
    dont_reenter: Option<Vec<PathBuf>>,
    #[cfg(feature = "std")]
    /// If inputs have been processed for multicore loading
    /// relevant only for `load_initial_inputs_multicore`
    multicore_inputs_processed: Option<bool>,
    /// The last time we reported progress (if available/used).
    /// This information is used by fuzzer `maybe_report_progress`.
    last_report_time: Option<Duration>,
    /// The last time something was added to the corpus
    last_found_time: Duration,
    /// The current index of the corpus; used to record for resumable fuzzing.
    corpus_id: Option<CorpusId>,
    /// Request the fuzzer to stop at the start of the next stage
    /// or at the beginning of the next fuzzing iteration
    stop_requested: bool,
    phantom: PhantomData<(I, SC)>,
}

/// The [`Testcase`] metadata.
#[derive(Serialize, Deserialize, Clone, Debug, Default, TypedBuilder)]
pub struct TestcaseMetadata {
    /// Map of metadata associated with this [`Testcase`]
    #[builder(default)]
    metadata: SerdeAnyMap,
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
    /// Parent [`CorpusId`], if known
    #[builder(default)]
    parent_id: Option<CorpusId>,
    /// If the testcase is "disabled"
    #[builder(default = false)]
    disabled: bool,
    /// has found crash (or timeout) or not
    #[builder(default = 0)]
    objectives_found: usize,
    /// Vector of `Feedback` names that deemed this `Testcase` as corpus worthy
    #[cfg(feature = "track_hit_feedbacks")]
    #[builder(default)]
    hit_feedbacks: Vec<Cow<'static, str>>,
    /// Vector of `Feedback` names that deemed this `Testcase` as solution worthy
    #[cfg(feature = "track_hit_feedbacks")]
    #[builder(default)]
    hit_objectives: Vec<Cow<'static, str>>,
}

impl HasMetadata for TestcaseMetadata {
    fn metadata_map(&self) -> &SerdeAnyMap {
        &self.metadata
    }

    fn metadata_map_mut(&mut self) -> &mut SerdeAnyMap {
        &mut self.metadata
    }
}

impl TestcaseMetadata {
    /// Get the executions
    #[inline]
    #[must_use]
    pub fn executions(&self) -> u64 {
        self.executions
    }

    /// Get the execution time of the testcase
    #[inline]
    #[must_use]
    pub fn exec_time(&self) -> &Option<Duration> {
        &self.exec_time
    }

    /// Get the `scheduled_count`
    #[inline]
    #[must_use]
    pub fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    /// Get `disabled`
    #[inline]
    #[must_use]
    pub fn disabled(&mut self) -> bool {
        self.disabled
    }

    /// Get the hit feedbacks
    #[inline]
    #[must_use]
    #[cfg(feature = "track_hit_feedbacks")]
    pub fn hit_feedbacks(&self) -> &Vec<Cow<'static, str>> {
        &self.hit_feedbacks
    }

    /// Get the hit objectives
    #[inline]
    #[must_use]
    #[cfg(feature = "track_hit_feedbacks")]
    pub fn hit_objectives(&self) -> &Vec<Cow<'static, str>> {
        &self.hit_objectives
    }

    /// Get the id of the parent, that this testcase was derived from
    #[must_use]
    pub fn parent_id(&self) -> Option<CorpusId> {
        self.parent_id
    }

    /// Gets how many objectives were found by mutating this testcase
    #[must_use]
    pub fn objectives_found(&self) -> usize {
        self.objectives_found
    }

    /// Get the executions (mutable)
    #[inline]
    #[must_use]
    pub fn executions_mut(&mut self) -> &mut u64 {
        &mut self.executions
    }

    /// Set the executions
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

    /// Set the `scheduled_count`
    #[inline]
    pub fn set_scheduled_count(&mut self, scheduled_count: usize) {
        self.scheduled_count = scheduled_count;
    }

    /// Set the testcase as disabled
    #[inline]
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Get the hit feedbacks (mutable)
    #[cfg(feature = "track_hit_feedbacks")]
    #[inline]
    #[must_use]
    pub fn hit_feedbacks_mut(&mut self) -> &mut Vec<Cow<'static, str>> {
        &mut self.hit_feedbacks
    }

    /// Get the hit objectives (mutable)
    #[cfg(feature = "track_hit_feedbacks")]
    #[inline]
    #[must_use]
    pub fn hit_objectives_mut(&mut self) -> &mut Vec<Cow<'static, str>> {
        &mut self.hit_objectives
    }

    /// Sets the id of the parent, that this testcase was derived from
    pub fn set_parent_id(&mut self, parent_id: CorpusId) {
        self.parent_id = Some(parent_id);
    }

    /// Sets the id of the parent, that this testcase was derived from
    pub fn set_parent_id_optional(&mut self, parent_id: Option<CorpusId>) {
        self.parent_id = parent_id;
    }

    /// Adds one objective to the `objectives_found` counter. Mostly called from crash handler or executor.
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

    /// Set the filename of the corpus input
    pub fn set_filename(&mut self, filename: TestcaseFilenameFormat) {
        self.filename_format = filename;
    }
}

/// The Metadata for each testcase used in power schedules.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(
    any(not(feature = "serdeany_autoreg"), miri),
    expect(clippy::unsafe_derive_deserialize)
)] // for SerdeAny
pub struct SchedulerTestcaseMetadata {
    /// Number of bits set in bitmap, updated in `calibrate_case`
    bitmap_size: u64,
    /// Number of queue cycles behind
    handicap: u64,
    /// Path depth, initialized in `on_add`
    depth: u64,
    /// Offset in `n_fuzz`
    n_fuzz_entry: usize,
    /// Cycles used to calibrate this (not really needed if it were not for `on_replace` and `on_remove`)
    cycle_and_time: (Duration, usize),
}

impl SchedulerTestcaseMetadata {
    /// Create new [`struct@SchedulerTestcaseMetadata`]
    #[must_use]
    pub fn new(depth: u64) -> Self {
        Self {
            bitmap_size: 0,
            handicap: 0,
            depth,
            n_fuzz_entry: 0,
            cycle_and_time: (Duration::default(), 0),
        }
    }

    /// Create new [`struct@SchedulerTestcaseMetadata`] given `n_fuzz_entry`
    #[must_use]
    pub fn with_n_fuzz_entry(depth: u64, n_fuzz_entry: usize) -> Self {
        Self {
            bitmap_size: 0,
            handicap: 0,
            depth,
            n_fuzz_entry,
            cycle_and_time: (Duration::default(), 0),
        }
    }

    /// Get the bitmap size
    #[inline]
    #[must_use]
    pub fn bitmap_size(&self) -> u64 {
        self.bitmap_size
    }

    /// Set the bitmap size
    #[inline]
    pub fn set_bitmap_size(&mut self, val: u64) {
        self.bitmap_size = val;
    }

    /// Get the handicap
    #[inline]
    #[must_use]
    pub fn handicap(&self) -> u64 {
        self.handicap
    }

    /// Set the handicap
    #[inline]
    pub fn set_handicap(&mut self, val: u64) {
        self.handicap = val;
    }

    /// Get the depth
    #[inline]
    #[must_use]
    pub fn depth(&self) -> u64 {
        self.depth
    }

    /// Set the depth
    #[inline]
    pub fn set_depth(&mut self, val: u64) {
        self.depth = val;
    }

    /// Get the `n_fuzz_entry`
    #[inline]
    #[must_use]
    pub fn n_fuzz_entry(&self) -> usize {
        self.n_fuzz_entry
    }

    /// Set the `n_fuzz_entry`
    #[inline]
    pub fn set_n_fuzz_entry(&mut self, val: usize) {
        self.n_fuzz_entry = val;
    }

    /// Get the cycles
    #[inline]
    #[must_use]
    pub fn cycle_and_time(&self) -> (Duration, usize) {
        self.cycle_and_time
    }

    #[inline]
    /// Setter for cycles
    pub fn set_cycle_and_time(&mut self, cycle_and_time: (Duration, usize)) {
        self.cycle_and_time = cycle_and_time;
    }
}

libafl_bolts::impl_serdeany!(SchedulerTestcaseMetadata);

#[cfg(feature = "std")]
impl<C, I, R, SC, SO> StdState<C, I, R, SC, SO>
where
    C: Corpus<I, SC>,
    I: Input,
    R: Rand,
    SO: Corpus<I, SC>,
{
    /// The max size allowed for the input
    fn max_size(&self) -> usize {
        self.max_size
    }

    /// Set the max size.
    fn max_size_mut(&mut self) -> &mut usize {
        &mut self.max_size
    }

    /// The executions counter
    fn executions(&self) -> u64 {
        self.executions
    }

    /// The executions counter (mutable)
    fn executions_mut(&mut self) -> &mut u64 {
        &mut self.executions
    }

    ///the imported testcases counter
    fn imported(&self) -> usize {
        self.imported
    }

    ///the imported testcases counter (mutable)
    fn imported_mut(&mut self) -> &mut usize {
        &mut self.imported
    }

    /// The starting time
    fn start_time(&self) -> &Duration {
        &self.start_time
    }

    /// The starting time (mutable)
    fn start_time_mut(&mut self) -> &mut Duration {
        &mut self.start_time
    }

    /// The last time we found something by ourselves
    fn last_found_time(&self) -> &Duration {
        &self.last_found_time
    }

    /// The last time we found something by ourselves (mutable)
    fn last_found_time_mut(&mut self) -> &mut Duration {
        &mut self.last_found_time
    }

    /// The last time we reported progress,if available/used.
    /// This information is used by fuzzer `maybe_report_progress`.
    fn last_report_time(&self) -> &Option<Duration> {
        &self.last_report_time
    }

    /// The last time we reported progress,if available/used (mutable).
    /// This information is used by fuzzer `maybe_report_progress`.
    fn last_report_time_mut(&mut self) -> &mut Option<Duration> {
        &mut self.last_report_time
    }

    /// The solutions corpus
    fn solutions(&self) -> &SO {
        &self.solutions
    }
    /// The solutions corpus (mutable)
    fn solutions_mut(&mut self) -> &mut SO {
        &mut self.solutions
    }

    /// Returns the corpus
    #[inline]
    fn corpus(&self) -> &C {
        &self.corpus
    }

    /// Returns the mutable corpus
    #[inline]
    fn corpus_mut(&mut self) -> &mut C {
        &mut self.corpus
    }

    /// The rand instance
    #[inline]
    fn rand(&self) -> &R {
        &self.rand
    }

    /// The rand instance (mutable)
    #[inline]
    fn rand_mut(&mut self) -> &mut R {
        &mut self.rand
    }

    /// To get the testcase
    fn testcase(&self, id: CorpusId) -> &Testcase<I> {
        self.corpus().get(id)?.borrow()
    }

    /// To get mutable testcase
    fn testcase_mut(&self, id: CorpusId) -> &mut Testcase<I> {
        self.corpus().get(id)?.borrow_mut()
    }

    /// Get all the metadata into an [`hashbrown::HashMap`]
    #[inline]
    fn named_metadata_map(&self) -> &NamedSerdeAnyMap {
        &self.named_metadata
    }

    /// Get all the metadata into an [`hashbrown::HashMap`] (mutable)
    #[inline]
    fn named_metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap {
        &mut self.named_metadata
    }

    fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    fn discard_stop_request(&mut self) {
        self.stop_requested = false;
    }

    fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    fn set_corpus_id(&mut self, id: CorpusId) -> Result<(), Error> {
        self.corpus_id = Some(id);
        Ok(())
    }

    fn current_corpus_id(&self) -> Result<Option<CorpusId>, Error> {
        Ok(self.corpus_id)
    }

    // to romain: are these 3 methods safely handling of the corpus?

    fn current_testcase(&self) -> Result<&Testcase<I>, Error> {
        let Some(corpus_id) = self.current_corpus_id()? else {
            return Err(Error::key_not_found(
                "We are not currently processing a testcase",
            ));
        };

        Ok(self.corpus().get(corpus_id)?.borrow())
    }

    fn current_testcase_mut(&self) -> Result<&mut Testcase<I>, Error> {
        let Some(corpus_id) = self.current_corpus_id()? else {
            return Err(Error::key_not_found(
                "We are not currently processing a testcase",
            ));
        };

        Ok(self.corpus().get(corpus_id)?.borrow_mut())
    }

    fn current_input_cloned(&self) -> Result<I, Error> {
        let mut testcase = self.current_testcase_mut()?;
        Ok(testcase.borrow_mut().load_input(self.corpus())?.clone()) // to romain
    }

    #[cfg(feature = "introspection")]
    fn introspection_stats(&self) -> &ClientPerfStats {
        &self.introspection_stats
    }

    #[cfg(feature = "introspection")]
    fn introspection_stats_mut(&mut self) -> &mut ClientPerfStats {
        &mut self.introspection_stats
    }

    /// Decide if the state must load the inputs
    pub fn must_load_initial_inputs(&self) -> bool {
        self.corpus().count() == 0
            || (self.remaining_initial_files.is_some()
                && !self.remaining_initial_files.as_ref().unwrap().is_empty())
    }

    /// List initial inputs from a directory.
    fn next_file(&mut self) -> Result<PathBuf, Error> {
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
    fn canonicalize_input_dirs(&mut self, in_dirs: &[PathBuf]) -> Result<(), Error> {
        if let Some(remaining) = self.remaining_initial_files.as_ref() {
            // everything was loaded
            if remaining.is_empty() {
                return Ok(());
            }
        } else {
            let files = in_dirs.iter().try_fold(Vec::new(), |mut res, file| {
                file.canonicalize().map(|canonicalized| {
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
    fn load_initial_inputs_custom_by_filenames<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        file_list: &[PathBuf],
        load_config: LoadConfig<I, Self, Z>,
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        if let Some(remaining) = self.remaining_initial_files.as_ref() {
            // everything was loaded
            if remaining.is_empty() {
                return Ok(());
            }
        } else {
            self.remaining_initial_files = Some(file_list.to_vec());
        }

        self.continue_loading_initial_inputs_custom(fuzzer, executor, load_config)
    }

    fn load_file<E, Z>(
        &mut self,
        path: &Path,
        fuzzer: &mut Z,
        executor: &mut E,
        config: &mut LoadConfig<I, Self, Z>,
    ) -> Result<ExecuteInputResult, Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        log::info!("Loading file {} ...", path.display());
        let input = match (config.loader)(fuzzer, self, path) {
            Ok(input) => input,
            Err(err) => {
                log::error!(
                    "Skipping input that we could not load from {}: {err:?}",
                    path.display()
                );
                return Ok(ExecuteInputResult::None);
            }
        };
        if config.forced {
            let _ = fuzzer.add_input(self, executor, input)?;
            Ok(ExecuteInputResult::Corpus)
        } else {
            let (res, _) = fuzzer.evaluate_input(self, executor, &input)?;
            if res == ExecuteInputResult::None {
                fuzzer.add_disabled_input(self, input)?;
                log::warn!(
                    "Input {} was not interesting, adding as disabled.",
                    path.display()
                );
            }
            Ok(res)
        }
    }
    /// Loads initial inputs from the passed-in `in_dirs`.
    /// This method takes a list of files and a `LoadConfig`
    /// which specifies the special handling of initial inputs
    fn continue_loading_initial_inputs_custom<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        mut config: LoadConfig<I, Self, Z>,
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        loop {
            match self.next_file() {
                Ok(path) => {
                    let res = self.load_file(&path, fuzzer, executor, &mut config)?;
                    if config.exit_on_solution && matches!(res, ExecuteInputResult::Solution) {
                        return Err(Error::invalid_corpus(format!(
                            "Input {} resulted in a solution.",
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
    pub fn walk_initial_inputs<F>(
        &mut self,
        in_dirs: &[PathBuf],
        mut closure: F,
    ) -> Result<(), Error>
    where
        F: FnMut(&PathBuf) -> Result<(), Error>,
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
    pub fn load_initial_inputs_by_filenames<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        file_list: &[PathBuf],
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        self.load_initial_inputs_custom_by_filenames(
            fuzzer,
            executor,
            file_list,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                forced: false,
                exit_on_solution: false,
            },
        )
    }

    /// Loads all intial inputs, even if they are not considered `interesting`.
    /// This is rarely the right method, use `load_initial_inputs`,
    /// and potentially fix your `Feedback`, instead.
    pub fn load_initial_inputs_forced<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        in_dirs: &[PathBuf],
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        self.continue_loading_initial_inputs_custom(
            fuzzer,
            executor,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                forced: true,
                exit_on_solution: false,
            },
        )
    }
    /// Loads initial inputs from the passed-in `in_dirs`.
    /// If `forced` is true, will add all testcases, no matter what.
    /// This method takes a list of files, instead of folders.
    pub fn load_initial_inputs_by_filenames_forced<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        file_list: &[PathBuf],
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        self.load_initial_inputs_custom_by_filenames(
            fuzzer,
            executor,
            file_list,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                forced: true,
                exit_on_solution: false,
            },
        )
    }

    /// Loads initial inputs from the passed-in `in_dirs`.
    pub fn load_initial_inputs<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        in_dirs: &[PathBuf],
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        self.continue_loading_initial_inputs_custom(
            fuzzer,
            executor,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                forced: false,
                exit_on_solution: false,
            },
        )
    }

    /// Loads initial inputs from the passed-in `in_dirs`.
    /// Will return a `CorpusError` if a solution is found
    pub fn load_initial_inputs_disallow_solution<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        in_dirs: &[PathBuf],
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        self.canonicalize_input_dirs(in_dirs)?;
        self.continue_loading_initial_inputs_custom(
            fuzzer,
            executor,
            LoadConfig {
                loader: &mut |_, _, path| I::from_file(path),
                forced: false,
                exit_on_solution: true,
            },
        )
    }

    fn calculate_corpus_size(&mut self) -> Result<usize, Error> {
        let mut count: usize = 0;
        loop {
            match self.next_file() {
                Ok(_) => {
                    count = count.saturating_add(1);
                }
                Err(Error::IteratorEnd(_, _)) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(count)
    }
    /// Loads initial inputs by dividing the from the passed-in `in_dirs`
    /// in a multicore fashion. Divides the corpus in chunks spread across cores.
    pub fn load_initial_inputs_multicore<E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        in_dirs: &[PathBuf],
        core_id: &CoreId,
        cores: &Cores,
    ) -> Result<(), Error>
    where
        Z: Evaluator<E, I, Self>,
    {
        if self.multicore_inputs_processed.unwrap_or(false) {
            self.continue_loading_initial_inputs_custom(
                fuzzer,
                executor,
                LoadConfig {
                    loader: &mut |_, _, path| I::from_file(path),
                    forced: false,
                    exit_on_solution: false,
                },
            )?;
        } else {
            self.canonicalize_input_dirs(in_dirs)?;
            let corpus_size = self.calculate_corpus_size()?;
            log::info!(
                "{} total_corpus_size, {} cores",
                corpus_size,
                cores.ids.len()
            );
            self.reset_initial_files_state();
            self.canonicalize_input_dirs(in_dirs)?;
            if cores.ids.len() > corpus_size {
                log::info!("low intial corpus count ({corpus_size}), no parallelism required.");
            } else {
                let core_index = cores
                    .ids
                    .iter()
                    .enumerate()
                    .find(|(_, c)| *c == core_id)
                    .unwrap_or_else(|| panic!("core id {} not in cores list", core_id.0))
                    .0;
                let chunk_size = corpus_size.saturating_div(cores.ids.len());
                let mut skip = core_index.saturating_mul(chunk_size);
                let mut inputs_todo = chunk_size;
                let mut collected_inputs = Vec::new();
                log::info!(
                    "core = {}, core_index = {}, chunk_size = {}, skip = {}",
                    core_id.0,
                    core_index,
                    chunk_size,
                    skip
                );
                loop {
                    match self.next_file() {
                        Ok(path) => {
                            if skip != 0 {
                                skip = skip.saturating_sub(1);
                                continue;
                            }
                            if inputs_todo == 0 {
                                break;
                            }
                            collected_inputs.push(path);
                            inputs_todo = inputs_todo.saturating_sub(1);
                        }
                        Err(Error::IteratorEnd(_, _)) => break,
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
                self.remaining_initial_files = Some(collected_inputs);
            }
            self.multicore_inputs_processed = Some(true);
            return self.load_initial_inputs_multicore(fuzzer, executor, in_dirs, core_id, cores);
        }
        Ok(())
    }
}

impl<C, I, R, SC, SO> StdState<C, I, R, SC, SO>
where
    C: Corpus<I, SC>,
    I: Input,
    R: Rand,
    SO: Corpus<I, SC>,
{
    fn generate_initial_internal<G, E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        generator: &mut G,
        num: usize,
        forced: bool,
    ) -> Result<(), Error>
    where
        G: Generator<I, Self>,
        Z: Evaluator<E, I, Self>,
    {
        let mut added = 0;
        for _ in 0..num {
            let input = generator.generate(self)?;
            if forced {
                let _ = fuzzer.add_input(self, executor, input)?;
                added += 1;
            } else {
                let input;
                let (res, _) = fuzzer.evaluate_input(self, executor, &input)?;
                if res != ExecuteInputResult::None {
                    added += 1;
                }
            }
        }
        Ok(())
    }

    /// Generate `num` initial inputs, using the passed-in generator and force the addition to corpus.
    pub fn generate_initial_inputs_forced<G, E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        generator: &mut G,
        num: usize,
    ) -> Result<(), Error>
    where
        G: Generator<I, Self>,
        Z: Evaluator<E, I, Self>,
    {
        self.generate_initial_internal(fuzzer, executor, generator, num, true)
    }

    /// Generate `num` initial inputs, using the passed-in generator.
    pub fn generate_initial_inputs<G, E, Z>(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        generator: &mut G,
        num: usize,
    ) -> Result<(), Error>
    where
        G: Generator<I, Self>,
        Z: Evaluator<E, I, Self>,
    {
        self.generate_initial_internal(fuzzer, executor, generator, num, false)
    }
}

impl<C, I, R, SC, SO> StdState<C, I, R, SC, SO>
where
    C: Corpus<I, SC>,
    I: Input,
    R: Rand,
    SO: Corpus<I, SC>,
{
    /// Creates a new `State`, taking ownership of all of the individual components during fuzzing.
    pub fn new<F, O>(
        rand: R,
        corpus: C,
        solutions: SO,
        feedback: &mut F,
        objective: &mut O,
    ) -> Result<Self, Error>
    where
        F: StateInitializer<Self>,
        O: StateInitializer<Self>,
        C: Serialize + DeserializeOwned,
        SO: Serialize + DeserializeOwned,
    {
        let mut state = Self {
            rand,
            executions: 0,
            imported: 0,
            start_time: libafl_bolts::current_time(),
            named_metadata: NamedSerdeAnyMap::default(),
            corpus,
            solutions,
            max_size: DEFAULT_MAX_SIZE,
            stop_requested: false,
            #[cfg(feature = "introspection")]
            introspection_stats: ClientPerfStats::new(),
            #[cfg(feature = "std")]
            remaining_initial_files: None,
            #[cfg(feature = "std")]
            dont_reenter: None,
            last_report_time: None,
            last_found_time: libafl_bolts::current_time(),
            corpus_id: None,
            phantom: PhantomData,
            #[cfg(feature = "std")]
            multicore_inputs_processed: None,
        };
        feedback.init_state(&mut state)?;
        objective.init_state(&mut state)?;
        Ok(state)
    }
}

impl StdState<InMemoryCorpus<NopInput>, NopInput, StdRand, NopScheduler, InMemoryCorpus<NopInput>> {
    /// Create an empty [`StdState`] that has very minimal uses.
    /// Potentially good for testing.
    pub fn nop() -> Result<
        StdState<InMemoryCorpus<NopInput>, NopInput, StdRand, InMemoryCorpus<NopInput>>,
        Error,
    > {
        StdState::new(
            StdRand::with_seed(0),
            InMemoryCorpus::<NopInput>::new(),
            InMemoryCorpus::new(),
            &mut (),
            &mut (),
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
    /// Create a new State that does nothing (for tests)
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
    use crate::state::StdState;

    #[test]
    fn test_std_state() {
        StdState::nop().expect("couldn't instantiate the test state");
    }
}
