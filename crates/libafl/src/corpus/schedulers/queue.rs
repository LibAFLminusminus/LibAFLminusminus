//! The queue corpus scheduler implements an AFL-like queue mechanism

use std::vec::Vec;

use alloc::borrow::ToOwned;
use serde::{Deserialize, Serialize};

use crate::{
    Error,
    corpus::{
        CorpusId, Scheduler, Testcase,
        schedulers::{HasQueueCycles, RemovableScheduler},
    },
    state::HasCorpus,
};

/// Walk the corpus in a queue-like fashion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueScheduler {
    queue: Vec<CorpusId>,
    current: Option<usize>,
    queue_cycles: u64,
    runs_in_current_cycle: u64,
}

impl<I, S> RemovableScheduler<I, S> for QueueScheduler {
    fn on_remove(
        &mut self,
        _state: &mut S,
        _id: CorpusId,
        _testcase: &Option<Testcase<I>>,
    ) -> Result<(), Error> {
        self.runs_in_current_cycle = self.runs_in_current_cycle.saturating_sub(1);
        Ok(())
    }
}

impl<I, S> Scheduler<I, S> for QueueScheduler
where
    S: HasCorpus<I, Self>,
{
    // fn on_add(&mut self, state: &mut S, id: CorpusId) -> Result<(), Error> {
    //     // Set parent id
    //     let current_id = state.current_corpus_id();

    //     state
    //         .corpus()
    //         .get(id)?
    //         .borrow_mut()
    //         .set_parent_id_optional(current_id);

    //     Ok(())
    // }
    fn on_add(&mut self, _state: &mut S, id: CorpusId) -> Result<(), Error> {
        log::warn!("what to do about parent id?");

        self.queue.push(id);

        Ok(())
    }

    fn current(&self, _state: &mut S) -> Option<CorpusId> {
        self.current.map(|idx| self.queue[idx].clone())
    }

    /// Gets the next entry in the queue
    fn next(&mut self, _state: &mut S) -> Result<CorpusId, Error> {
        if self.queue.is_empty() {
            Err(Error::empty("Scheduler queue is empty.".to_owned()))
        } else {
            let idx = if let Some(current) = &mut self.current {
                *current += 1;
                self.runs_in_current_cycle += 1;

                if *current >= self.queue.len() {
                    *current = 0;
                    self.queue_cycles += 1;
                    self.runs_in_current_cycle = 0;
                }

                *current
            } else {
                let idx = 0;
                self.current = Some(idx);

                debug_assert!(idx < self.queue.len());

                idx
            };

            Ok(self.queue[idx])
        }
    }
}

impl QueueScheduler {
    /// Creates a new `QueueScheduler`
    #[must_use]
    pub fn new() -> Self {
        Self {
            runs_in_current_cycle: 0,
            queue_cycles: 0,
            current: None,
            queue: Vec::new(),
        }
    }
}

impl Default for QueueScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl HasQueueCycles for QueueScheduler {
    fn queue_cycles(&self) -> u64 {
        self.queue_cycles
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
#[cfg(not(feature = "remove_me"))]
mod tests {

    use std::path::PathBuf;

    use libafl_bolts::rands::StdRand;

    use crate::{
        corpus::{
            Corpus, InMemoryCorpus, OnDiskCorpus,
            schedulers::{NopScheduler, QueueScheduler, Scheduler},
        },
        feedbacks::ConstFeedback,
        inputs::bytes::BytesInput,
        state::StdState,
    };

    #[test]
    fn test_queuecorpus() {
        let rand = StdRand::with_seed(4);
        let scheduler: QueueScheduler = QueueScheduler::new();

        let corpus = OnDiskCorpus::<BytesInput, QueueScheduler>::new(
            PathBuf::from("target/.test/fancy/path"),
            scheduler,
        )
        .unwrap();
        // let t = Testcase::with_filename(BytesInput::new(vec![0_u8; 4]), "fancyfile".into());
        // q.add(t).unwrap();

        let objective = OnDiskCorpus::<BytesInput, NopScheduler>::new(
            PathBuf::from("target/.test/fancy/objective/path"),
            NopScheduler,
        )
        .unwrap();

        let mut feedback = ConstFeedback::new(false);
        let mut objective_fb = ConstFeedback::new(false);

        let _state =
            StdState::new(rand, corpus, objective, &mut feedback, &mut objective_fb).unwrap();

        // let filename = state
        //     .corpus()
        //     .get(next_id)
        //     .unwrap()
        //     .borrow()
        //     .filename()
        //     .as_ref()
        //     .unwrap()
        //     .clone();
        // assert_eq!(filename, "fancyfile");

        // fs::remove_dir_all("target/.test/fancy/path").unwrap();
    }

    #[test]
    fn test_queue_scheduler_removal() {
        let rand = StdRand::with_seed(42);
        let mut scheduler = QueueScheduler::new();

        let mut q = InMemoryCorpus::<BytesInput, QueueScheduler>::new(scheduler);
        let t1 = BytesInput::new(vec![0_u8; 4]);
        let t2 = BytesInput::new(vec![0_u8; 4]);
        let t3 = BytesInput::new(vec![0_u8; 4]);

        let id1 = q.add(t1).unwrap();
        let id2 = q.add(t2).unwrap();
        let id3 = q.add(t3).unwrap();

        let mut feedback = ConstFeedback::new(false);
        let mut objective = ConstFeedback::new(false);

        let mut state = StdState::new(
            rand,
            q,
            InMemoryCorpus::new(NopScheduler),
            &mut feedback,
            &mut objective,
        )
        .unwrap();

        let next_id = state.scheduler_mut().next(&mut state).unwrap();
        assert_eq!(next_id, id1);
        assert_eq!(state.scheduler().runs_in_current_cycle, 1);

        let next_id = scheduler.next(&mut state).unwrap();
        assert_eq!(next_id, id2);

        assert_eq!(
            state.scheduler().queue_cycles,
            0,
            "Cycle finished prematurely!"
        );

        let next_id = scheduler.next(&mut state).unwrap();
        assert_eq!(next_id, id3);

        assert_eq!(state.scheduler().queue_cycles, 1);
    }
}
