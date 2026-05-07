use crate::{
    DependencyResolver, Error, Result, Worker,
    corpus::{Corpus, Testcase, schedulers::PowerScheduleData},
    executors::Executor,
    feedbacks::{HasObserverHandle, MapFeedbackMetadata},
    fuzzers::{ExitKind, FuzzerHook},
    inputs::Input,
    observers::{MapObserver, ObserversTuple},
    states::{
        FlatState, HasCorpus, has_named_metadata, has_unnamed_metadata, named_metadata_mut,
        unnamed_metadata_mut,
    },
};
use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
    vec::Vec,
};
use core::{marker::PhantomData, time::Duration};
use hashbrown::HashSet;
use libafl_bolts::{Named, current_time, impl_serdeany, tuples::Handle};
use num_traits::Bounded;
use serde::{Deserialize, Serialize};

/// AFL++'s `CAL_CYCLES_FAST` + 1
const CAL_STAGE_START: usize = 4;
/// AFL++'s `CAL_CYCLES` + 1
const CAL_STAGE_MAX: usize = 8;

/// Default name for `CalibrationStage`; derived from AFL++
pub const CALIBRATION_STAGE_NAME: &str = "calibration";

/// The metadata to keep unstable entries
/// Formula is same as AFL++: number of unstable entries divided by the number of filled entries.
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnstableEntriesMetadata {
    unstable_entries: HashSet<usize>,
    filled_entries_count: usize,
}
impl_serdeany!(UnstableEntriesMetadata);

impl UnstableEntriesMetadata {
    #[must_use]
    /// Create a new [`struct@UnstableEntriesMetadata`]
    pub fn new() -> Self {
        Self {
            unstable_entries: HashSet::new(),
            filled_entries_count: 0,
        }
    }

    /// Getter
    #[must_use]
    pub fn unstable_entries(&self) -> &HashSet<usize> {
        &self.unstable_entries
    }

    /// Getter
    #[must_use]
    pub fn filled_entries_count(&self) -> usize {
        self.filled_entries_count
    }
}

impl Default for UnstableEntriesMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs the target with pre and post execution hooks and returns the exit kind and duration.
pub fn run_target_measuring_time<E, I, S, W>(
    executor: &mut E,
    state: &mut S,
    rt_handle: &mut crate::runtimes::RuntimeHandle<S, W>,
    testcase: &Testcase<I>,
) -> Result<(ExitKind, Duration)>
where
    E: Executor<I, S>,
    I: Input,
    S: FlatState,
    W: Worker,
{
    executor.observers_mut().pre_exec_all(state)?;

    let start = current_time();
    let exit_kind = executor.execute(state, rt_handle, &testcase.input())?;
    let duration = current_time().checked_sub(start).ok_or_else(|| {
        Error::illegal_state(format!(
            "The time seems to have jumped in CalibrationStage! {start:?}"
        ))
    })?;

    executor.observers_mut().post_exec_all(state, &exit_kind)?;

    Ok((exit_kind, duration))
}

pub struct CalibrationHook<C, O> {
    /// the maximum number of times that we execute the harness for executions.
    stage_max: usize,
    name: Cow<'static, str>,
    map_observer_handle: Handle<C>,
    map_name: Cow<'static, str>,
    phantom: PhantomData<O>,
}

impl<C, O> DependencyResolver for CalibrationHook<C, O> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        registrator.register_md_default::<UnstableEntriesMetadata>(self.name().to_string());
        Ok(())
    }
}

impl<C, O> CalibrationHook<C, O> {
    pub fn new<F>(map_feedback: &F) -> Self
    where
        F: HasObserverHandle<Observer = C> + Named,
    {
        let map_name = map_feedback.name();
        Self {
            stage_max: CAL_STAGE_MAX,
            map_name: map_name.clone(),
            map_observer_handle: map_feedback.observer_handle().clone(),
            name: Cow::Owned(format!("calibration_{}", map_name.clone())),
            phantom: PhantomData,
        }
    }
}

/// Default name prefix for `CalibrationHook`; derived from AFL++
pub const CALIBRATION_HOOK_NAME: &str = "calibration";

#[derive(Debug, Serialize, Deserialize)]
pub struct StabilityValue(f64);

impl_serdeany!(StabilityValue);

impl<C, O> Named for CalibrationHook<C, O> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<C, E, I, O, S, W> FuzzerHook<E, I, S, W> for CalibrationHook<C, O>
where
    C: AsRef<O>,
    E: Executor<I, S>,
    I: Input,
    O: MapObserver,
    O::Entry: Serialize,
    for<'de> O::Entry: Deserialize<'de> + 'static + Default + Bounded,
    S: HasCorpus<I> + FlatState,
    W: Worker,
{
    fn post_add(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut crate::runtimes::RuntimeHandle<S, W>,
        testcase_id: crate::corpus::TestcaseId,
    ) -> Result<()> {
        let testcase = state.corpus().get(&testcase_id)?;

        // first run
        let (_, mut total_time) = run_target_measuring_time(executor, state, rt_handle, &testcase)?;

        let observers = &executor.observers();
        let map_first = observers[&self.map_observer_handle].as_ref();
        let map_first_filled_count = match state
            .named_metadata_map()
            .get::<MapFeedbackMetadata<O::Entry>>(&self.map_name)
        {
            Some(metadata) => metadata.num_covered_map_indexes,
            None => map_first.count_bytes().try_into().map_err(|len| {
                Error::illegal_state(
                    format!(
                        "map's filled entry count ({}) is greater than usize::MAX ({})",
                        len,
                        usize::MAX,
                    )
                    .as_str(),
                )
            })?,
        };
        let map_first_entries = map_first.to_vec();
        let map_first_len = map_first.to_vec().len();
        let mut unstable_entries: Vec<usize> = vec![];

        // Run CAL_STAGE_START - 1 times, increase by 2 for every time a new
        let mut i = 1;
        let mut iter = self.stage_max;

        // repeated runs
        while i < self.stage_max {
            let (exit_kind, duration) =
                run_target_measuring_time(executor, state, rt_handle, &testcase)?;

            total_time += duration;

            if exit_kind != ExitKind::Timeout {
                let map = &executor.observers()[&self.map_observer_handle]
                    .as_ref()
                    .to_vec();

                let map_state = state
                    .named_metadata_map_mut()
                    .get_mut::<MapFeedbackMetadata<O::Entry>>(&self.map_name)
                    .unwrap();
                let history_map = &mut map_state.history_map;

                if history_map.len() < map_first_len {
                    history_map.resize(map_first_len, O::Entry::default());
                }

                for (idx, (first, (cur, history))) in map_first_entries
                    .iter()
                    .zip(map.iter().zip(history_map.iter_mut()))
                    .enumerate()
                {
                    if *first != *cur && *history != O::Entry::max_value() {
                        // If we just hit a history map entry that was not covered before, but is now flagged as flaky,
                        // we need to make sure the `num_covered_map_indexes` is kept in sync.
                        map_state.num_covered_map_indexes +=
                            usize::from(*history == O::Entry::default());
                        *history = O::Entry::max_value();
                        unstable_entries.push(idx);
                    }
                }

                if !unstable_entries.is_empty() && iter < CAL_STAGE_MAX {
                    iter += 2;
                }
            }
            i += 1;
        }

        let unstable_found = !unstable_entries.is_empty();
        let stability = if unstable_found {
            let metadata = named_metadata_mut::<UnstableEntriesMetadata>(
                state.named_metadata_map_mut(),
                self.name(),
            )?;

            let unstable = unstable_entries.len();
            let all = map_first_filled_count;
            let stability = unstable as f64 / all as f64;

            // If we see new unstable entries executing this new corpus entries, then merge with the existing one
            for item in unstable_entries {
                metadata.unstable_entries.insert(item); // Insert newly found items
            }
            metadata.filled_entries_count = map_first_filled_count;

            stability
        } else {
            100.0f64
        };

        state
            .stats_mut()
            .user_map
            .insert("stability", StabilityValue(stability));

        if has_unnamed_metadata::<PowerScheduleData>(state.named_metadata_map()) {
            let psdata = unnamed_metadata_mut::<PowerScheduleData>(state.named_metadata_map_mut())?;

            let observers = executor.observers();
            let map = observers[&self.map_observer_handle].as_ref();
            let bitmap_size = map.count_bytes();

            if bitmap_size < 1 {
                return Err(Error::invalid_corpus(
                    "This testcase does not trigger any edges. Check your instrumentation!"
                        .to_string(),
                ));
            }

            let handicap = psdata.queue_cycles();

            // setting global power schedule data
            psdata.set_exec_time(psdata.exec_time() + total_time);
            psdata.set_cycles(psdata.cycles() + (iter as u64));
            psdata.set_bitmap_size(psdata.bitmap_size() + bitmap_size);
            psdata.set_bitmap_size_log(psdata.bitmap_size_log() + libm::log2(bitmap_size as f64));
            psdata.set_bitmap_entries(psdata.bitmap_entries() + 1);
        }

        Ok(())
    }
}
