//! The [`MinimizerScheduler`]`s` are a family of corpus schedulers that feed the fuzzer
//! with [`Testcase`]`s` only from a subset of the total [`Corpus`].

use core::{any::type_name, marker::PhantomData};

use hashbrown::{HashMap, HashSet};
use libafl_bolts::{AsIter, HasRefCnt, rands::Rand, serdeany::SerdeAny, tuples::MatchName};
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver, Error,
    corpus::{
        Corpus, CorpusId,
        schedulers::{LenTimeMulTestcasePenalty, RemovableScheduler, Scheduler, TestcasePenalty},
    },
    feedbacks::MapIndexesMetadata,
    observers::CanTrack,
    state::{HasCorpus, HasRand},
};

/// Default probability to skip the non-favored values
pub const DEFAULT_SKIP_NON_FAVORED_PROB: f64 = 0.95;

/// A testcase metadata saying if a testcase is favored
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(
    any(not(feature = "serdeany_autoreg"), miri),
    expect(clippy::unsafe_derive_deserialize)
)] // for SerdeAny
pub struct IsFavoredMetadata {}

libafl_bolts::impl_serdeany!(IsFavoredMetadata);

/// A state metadata holding a map of favoreds testcases for each map entry
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(
    any(not(feature = "serdeany_autoreg"), miri),
    expect(clippy::unsafe_derive_deserialize)
)] // for SerdeAny
pub struct TopRated {
    /// map index -> corpus index
    pub map: HashMap<usize, CorpusId>,
}

libafl_bolts::impl_serdeany!(TopRated);

impl TopRated {
    /// Creates a new [`struct@TopRatedsMetadata`]
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::default(),
        }
    }

    /// Getter for map
    #[must_use]
    pub fn map(&self) -> &HashMap<usize, CorpusId> {
        &self.map
    }
}

impl Default for TopRated {
    fn default() -> Self {
        Self::new()
    }
}

/// The [`MinimizerScheduler`] employs a genetic algorithm to compute a subset of the
/// corpus that exercise all the requested features.
///
/// E.g., it can use all the coverage seen so far to prioritize [`Testcase`]`s` using a [`TestcasePenalty`].
#[derive(Debug, Clone)]
pub struct MinimizerScheduler<CS, F, I, M, S> {
    base: CS,
    skip_non_favored_prob: f64,
    remove_metadata: bool,
    top_rated: TopRated,
    current: Option<CorpusId>,
    phantom: PhantomData<(F, I, M, S)>,
}

impl<CS, F, I, M, O> DependencyResolver for MinimizerScheduler<CS, F, I, M, O> {}

impl<CS, F, I, M, O, S> Scheduler<S> for MinimizerScheduler<CS, F, I, M, O>
where
    CS: Scheduler<S>,
    F: TestcasePenalty<I, S>,
    M: for<'a> AsIter<'a, Item = usize> + SerdeAny + HasRefCnt,
    S: HasRand,
{
    fn current(&self, _state: &mut S) -> Option<CorpusId> {
        self.current
    }

    /// Called when a [`Testcase`] is added to the corpus
    fn on_add(&mut self, state: &mut S, id: CorpusId) -> Result<(), Error> {
        self.base.on_add(state, id)?;
        self.update_score(state, id)
    }

    /// An input has been evaluated
    fn on_evaluation<OT>(&mut self, state: &mut S, observers: &OT) -> Result<(), Error>
    where
        OT: MatchName,
    {
        self.base.on_evaluation(state, observers)
    }

    /// Gets the next entry
    fn next(&mut self, state: &mut S) -> Result<CorpusId, Error> {
        self.cull(state)?;
        let mut id = self.base.next(state)?;
        while {
            state.testcase()
                .borrow()
                .has_metadata::<IsFavoredMetadata>()
        } && state.rand_mut().coinflip(self.skip_non_favored_prob)
        {
            id = self.base.next(state)?;
        }
        Ok(id)
    }
}

impl<CS, F, I, M, O> MinimizerScheduler<CS, F, I, M, O>
where
    M: for<'a> AsIter<'a, Item = usize> + SerdeAny + HasRefCnt,
{
    /// Update the [`Corpus`] score using the [`MinimizerScheduler`]
    #[expect(clippy::cast_possible_wrap)]
    pub fn update_score<S>(&self, state: &mut S, id: CorpusId) -> Result<(), Error>
    where
        F: TestcasePenalty<I, S>,
    {
        let mut new_favoreds = vec![];
        {
            let mut entry = state.corpus().get(id)?.borrow_mut();
            let factor = F::compute(state, &mut *entry)?;
            let meta = entry.metadata_map_mut().get_mut::<M>().ok_or_else(|| {
                Error::key_not_found(format!(
                    "Metadata needed for MinimizerScheduler not found in testcase #{id}"
                ))
            })?;
            let top_rateds = self.top_rated;
            for elem in meta.as_iter() {
                if let Some(old_id) = top_rateds.map.get(&*elem) {
                    if *old_id == id {
                        new_favoreds.push(*elem); // always retain current; we'll drop it later otherwise
                        continue;
                    }
                    match state.corpus().get(*old_id) {
                        Ok(testcase) => {
                            let mut old = testcase.borrow_mut();
                            if factor > F::compute(state, &mut *old)? {
                                continue;
                            }

                            let must_remove = {
                                let old_meta = old.metadata_map_mut().get_mut::<M>().ok_or_else(|| {
                                Error::key_not_found(format!(
                                    "{} needed for MinimizerScheduler not found in testcase #{old_id}",
                                    type_name::<M>()
                                ))
                            })?;
                                *old_meta.refcnt_mut() -= 1;
                                old_meta.refcnt() <= 0
                            };

                            if must_remove && self.remove_metadata {
                                drop(old.metadata_map_mut().remove::<M>());
                            }
                        }
                        Err(Error::KeyNotFound(_, _)) => {
                            log::warn!("Corpus entry {old_id} not found");
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }

                new_favoreds.push(*elem);
            }

            *meta.refcnt_mut() = new_favoreds.len() as isize;
        }

        if new_favoreds.is_empty() && self.remove_metadata {
            drop(
                state
                    .corpus()
                    .get(id)?
                    .borrow_mut()
                    .metadata_map_mut()
                    .remove::<M>(),
            );
            return Ok(());
        }

        for elem in new_favoreds {
            self.top_rated.map.insert(elem, id);
        }
        Ok(())
    }

    /// Cull the [`Corpus`] using the [`MinimizerScheduler`]
    pub fn cull<S>(&mut self, state: &mut S) -> Result<(), Error>
    where
        S: HasRand,
        CS: RemovableScheduler<I, S>,
        CS: Scheduler<S>,
        F: TestcasePenalty<I, S>,
    {
        let mut acc = HashSet::new();

        for (key, id) in &self.top_rated.map {
            if !acc.contains(key) {
                let entry = state.corpus().get(*id);
                match entry {
                    Ok(entry) => {
                        let mut entry = entry.borrow_mut();
                        let meta = entry.metadata_map().get::<M>().ok_or_else(|| {
                            Error::key_not_found(format!(
                                "{} needed for MinimizerScheduler not found in testcase #{id}",
                                type_name::<M>()
                            ))
                        })?;
                        for elem in meta.as_iter() {
                            acc.insert(*elem);
                        }

                        entry.add_metadata(IsFavoredMetadata {});
                    }
                    Err(Error::KeyNotFound(_, _)) => {
                        return self.cull(state);
                    }
                    Err(err) => {
                        return Err(err);
                    }
                }
            }
        }

        Ok(())
    }
}

impl<CS, F, I, M, O> MinimizerScheduler<CS, F, I, M, O>
where
    O: CanTrack,
{
    /// Get a reference to the base scheduler
    pub fn base(&self) -> &CS {
        &self.base
    }

    /// Get a reference to the base scheduler (mut)
    pub fn base_mut(&mut self) -> &mut CS {
        &mut self.base
    }

    /// Creates a new [`MinimizerScheduler`] that wraps a `base` [`Scheduler`]
    /// and has a default probability to skip non-faved [`Testcase`]s of [`DEFAULT_SKIP_NON_FAVORED_PROB`].
    /// This will remove the metadata `M` when it is no longer needed, after consumption. This might
    /// for example be a `MapIndexesMetadata`.
    ///
    /// When calling, pass the edges observer which will provided the indexes to minimize over.
    pub fn new(_observer: &O, base: CS) -> Self {
        Self {
            base,
            skip_non_favored_prob: DEFAULT_SKIP_NON_FAVORED_PROB,
            remove_metadata: true,
            current: None,
            top_rated: TopRated::new(),
            phantom: PhantomData,
        }
    }

    /// Creates a new [`MinimizerScheduler`] that wraps a `base` [`Scheduler`]
    /// and has a default probability to skip non-faved [`Testcase`]s of [`DEFAULT_SKIP_NON_FAVORED_PROB`].
    /// This method will prevent the metadata `M` from being removed at the end of scoring.
    ///
    /// When calling, pass the edges observer which will provided the indexes to minimize over.
    pub fn non_metadata_removing(_observer: &O, base: CS) -> Self {
        Self {
            base,
            skip_non_favored_prob: DEFAULT_SKIP_NON_FAVORED_PROB,
            remove_metadata: false,
            top_rated: TopRated::new(),
            current: None,
            phantom: PhantomData,
        }
    }

    /// Creates a new [`MinimizerScheduler`] that wraps a `base` [`Scheduler`]
    /// and has a non-default probability to skip non-faved [`Testcase`]s using (`skip_non_favored_prob`).
    ///
    /// When calling, pass the edges observer which will provided the indexes to minimize over.
    pub fn with_skip_prob(_observer: &O, base: CS, skip_non_favored_prob: f64) -> Self {
        Self {
            base,
            skip_non_favored_prob,
            remove_metadata: true,
            top_rated: TopRated::new(),
            current: None,
            phantom: PhantomData,
        }
    }
}

/// A [`MinimizerScheduler`] with [`LenTimeMulTestcasePenalty`] to prioritize quick and small [`Testcase`]`s`.
pub type LenTimeMinimizerScheduler<CS, I, M, O> =
    MinimizerScheduler<CS, LenTimeMulTestcasePenalty, I, M, O>;

/// A [`MinimizerScheduler`] with [`LenTimeMulTestcasePenalty`] to prioritize quick and small [`Testcase`]`s`
/// that exercise all the entries registered in the [`MapIndexesMetadata`].
pub type IndexesLenTimeMinimizerScheduler<CS, I, O> =
    MinimizerScheduler<CS, LenTimeMulTestcasePenalty, I, MapIndexesMetadata, O>;

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use libafl_bolts::rands::StdRand;

    use crate::{
        corpus::{
            Corpus, InMemoryCorpus, Testcase,
            schedulers::{
                IndexesLenTimeMinimizerScheduler, MinimizerScheduler, QueueScheduler, Scheduler,
            },
        },
        feedbacks::MapIndexesMetadata,
        inputs::NopInput,
        observers::{CanTrack, StdMapObserver},
        state::{HasCorpus, StdState},
    };

    #[test]
    fn test_minimizer_scheduler_update_score_crash() {
        #[cfg(not(feature = "serdeany_autoreg"))]
        unsafe {
            libafl_bolts::serdeany::RegistryBuilder::register::<TopRatedsMetadata>();
            libafl_bolts::serdeany::RegistryBuilder::register::<MapIndexesMetadata>();
        }
        let rand = StdRand::with_seed(0);
        let observer = StdMapObserver::owned("map", vec![0u8; 16]).track_indices();
        let mut scheduler: IndexesLenTimeMinimizerScheduler<QueueScheduler, NopInput, _> =
            MinimizerScheduler::new(&observer, QueueScheduler::new());

        let mut corpus = InMemoryCorpus::new();
        let t1 = Testcase::new(Rc::new(NopInput));
        let _id1 = corpus.add(t1).unwrap();

        let mut state =
            StdState::new(rand, corpus, InMemoryCorpus::new()).unwrap();

        state.add_metadata(TopRatedsMetadata::new());
        let top_rateds = state.metadata_mut::<TopRatedsMetadata>().unwrap();

        top_rateds.map.insert(0, 999_usize.into());

        let mut t2 = Testcase::new(Rc::new(NopInput));
        let map_meta = MapIndexesMetadata::new(vec![0]);
        t2.add_metadata(map_meta);
        let id2 = state.corpus_mut().add(t2).unwrap();

        scheduler.on_add(&mut state, id2).unwrap();
    }
}
