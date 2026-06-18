//! The queue corpus scheduler implements an AFL-like queue mechanism

use crate::common::DependencyResolver;
use crate::{
    Error,
    corpus::{Scheduler, testcase::TestcaseId},
};
use alloc::{borrow::ToOwned, vec::Vec};
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

/// Walk the corpus in a queue-like fashion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueScheduler {
    queue: Vec<TestcaseId>,
    current: Option<usize>,
}

impl DependencyResolver for QueueScheduler {}

impl Scheduler for QueueScheduler {
    fn on_add(&mut self, id: TestcaseId) -> Result<()> {
        self.queue.push(id);

        Ok(())
    }

    fn current(&self) -> Option<TestcaseId> {
        self.current.map(|idx| self.queue[idx])
    }

    /// Gets the next entry in the queue
    fn next(&mut self) -> Result<TestcaseId> {
        if self.queue.is_empty() {
            Err(Error::empty("Scheduler queue".to_owned()))
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

    fn ids(&self) -> &[TestcaseId] {
        &self.queue
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
mod tests {
    use crate::{
        corpus::{
            InMemoryCorpus, ObjectiveInMemoryCorpus, ObjectiveOnDiskCorpus, ScheduledCorpus,
            Testcase,
            collection::OnDiskCorpus,
            schedulers::{QueueScheduler, Scheduler},
        },
        inputs::bytes::{BytesContext, BytesInput},
        states::{State, StdState},
    };
    use alloc::rc::Rc;
    use std::path::PathBuf;

    #[test]
    fn test_queue_corpus() {
        let scheduler: QueueScheduler = QueueScheduler::new();
        let context = BytesContext;

        let corpus = OnDiskCorpus::builder(scheduler)
            .root_dir(PathBuf::from("/tmp"))
            .build()
            .unwrap();

        let objective = ObjectiveOnDiskCorpus::builder()
            .root_dir(PathBuf::from("/tmp"))
            .build()
            .unwrap();

        let _state = StdState::new(context, corpus, objective).unwrap();

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
    fn test_queue_scheduler() {
        let scheduler = QueueScheduler::new();
        let context = BytesContext;

        let mut q = InMemoryCorpus::new(scheduler);
        let t1 = BytesInput::new(vec![0_u8; 4]);
        let t2 = BytesInput::new(vec![1_u8; 4]);
        let t3 = BytesInput::new(vec![2_u8; 4]);

        let id1 = q.add(Testcase::new(Rc::new(t1))).unwrap();
        let id2 = q.add(Testcase::new(Rc::new(t2))).unwrap();
        let id3 = q.add(Testcase::new(Rc::new(t3))).unwrap();

        let mut state = StdState::new(context, q, ObjectiveInMemoryCorpus::new()).unwrap();

        let next_id = state.corpus_mut().scheduler_mut().next().unwrap();
        assert_eq!(next_id, id1);

        let next_id = state.corpus_mut().scheduler_mut().next().unwrap();
        assert_eq!(next_id, id2);

        let next_id = state.corpus_mut().scheduler_mut().next().unwrap();
        assert_eq!(next_id, id3);
    }
}
