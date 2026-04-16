//! The queue corpus scheduler implements an AFL-like queue mechanism

use alloc::borrow::ToOwned;
use std::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver, Error,
    corpus::{CorpusId, Scheduler},
};

/// Walk the corpus in a queue-like fashion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueScheduler {
    queue: Vec<CorpusId>,
    current: Option<usize>,
}

impl DependencyResolver for QueueScheduler {}

impl<S> Scheduler<S> for QueueScheduler {
    fn on_add(&mut self, _state: &mut S, id: CorpusId) -> Result<(), Error> {
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
            let idx = if let Some(current) = self.current {
                if self.queue.get(current + 1).is_some() {
                    current + 1
                } else {
                    0
                }
            } else {
                0
            };
            self.current = Some(idx);
            Ok(self.queue[idx])
        }
    }
}

impl QueueScheduler {
    /// Creates a new `QueueScheduler`
    #[must_use]
    pub fn new() -> Self {
        Self {
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

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {

    use std::path::PathBuf;

    use libafl_bolts::rands::StdRand;

    use crate::{
        corpus::{
            Corpus, InMemoryCorpus, OnDiskCorpus,
            schedulers::{NopScheduler, QueueScheduler, Scheduler},
        },
        inputs::bytes::BytesInput,
        state::{HasCorpus, StdState},
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

        let _state =
            StdState::new(rand, corpus, objective).unwrap();

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

        let mut state = StdState::new(
            rand,
            q,
            InMemoryCorpus::new(NopScheduler),
        )
        .unwrap();

        let next_id = state.corpus_mut().scheduler_mut().next(&mut state).unwrap();
        assert_eq!(next_id, id1);

        let next_id = scheduler.next(&mut state).unwrap();
        assert_eq!(next_id, id2);

        let next_id = scheduler.next(&mut state).unwrap();
        assert_eq!(next_id, id3);
    }
}
