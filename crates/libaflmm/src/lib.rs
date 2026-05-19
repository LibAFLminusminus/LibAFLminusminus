/*!
Welcome to `LibAFL`
*/
#![doc = include_str!("../README.md")]
/*! */
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

pub extern crate alloc;

// Re-export derive(SerdeAny)
#[cfg(feature = "derive")]
#[allow(unused_imports)] // cfg-dependent
#[macro_use]
extern crate libaflmm_derive;
#[cfg(feature = "derive")]
#[doc(hidden)]
pub use libaflmm_derive::*;

pub mod common;
pub use common::*;
pub mod controllers;
pub use controllers::*;
pub mod corpus;
pub mod executors;
pub mod feedbacks;
pub mod fuzzers;
pub use fuzzers::*;
pub mod generators;
pub mod inputs;
pub mod launchers;
pub mod monitors;
pub mod mutators;
pub mod observers;
pub mod runtimes;
pub mod stages;
pub mod states;

pub use libaflmm_core::{Error, Result};
pub use libaflmm_core::{non_zero, non_zero_const};

/// The purpose of this module is to alleviate imports of many components by adding a glob import.
#[cfg(feature = "prelude")]
pub mod prelude {
    #![expect(ambiguous_glob_reexports)]

    pub use super::{
        corpus::*, executors::*, feedbacks::*, fuzzers::*, generators::*, inputs::*, monitors::*,
        mutators::*, observers::*, runtimes::*, stages::*, states::*,
    };
}

// TODO: adapt this test...
// #[cfg(test)]
// mod tests {
//
//     #[cfg(miri)]
//     use libaflmm_bolts::serdeany::RegistryBuilder;
//     use libaflmm_bolts::{
//         rands::{RomuDuoJrRand, StdRand},
//         tuples::tuple_list,
//     };
//     use serial_test::serial;
//
//     #[cfg(miri)]
//     use crate::stages::ExecutionCountRestartHelperMetadata;
//     use crate::{
//         StdFuzzer,
//         corpus::{Corpus, InMemoryCorpus, Testcase},
//         executors::ExitKind,
//         feedbacks::ConstFeedback,
//         fuzzers::Fuzzer,
//         inputs::BytesInput,
//         monitors::SimpleMonitor,
//         mutators::{HavocScheduledMutator, mutations::BitFlipMutator},
//         stages::StdMutationalStage,
//         states::{HasCorpus, StdState},
//     };
//
//     #[test]
//     #[serial]
//     fn test_fuzzer() {
//         // # Safety
//         // No concurrency per testcase
//         #[cfg(miri)]
//         unsafe {
//             RegistryBuilder::register::<ExecutionCountRestartHelperMetadata>();
//         }
//
//         let rand = StdRand::with_seed(0);
//
//         let mut corpus = InMemoryCorpus::<BytesInput>::new();
//         let testcase = Testcase::new(vec![0; 4].into());
//         corpus.add(testcase).unwrap();
//
//         let mut feedback = ConstFeedback::new(false);
//         let mut objective = ConstFeedback::new(false);
//
//         let mut state = StdState::new(
//             rand,
//             corpus,
//             InMemoryCorpus::<BytesInput>::new(),
//             &mut feedback,
//             &mut objective,
//         )
//         .unwrap();
//
//         let _monitor = SimpleMonitor::new(|s| {
//             println!("{s}");
//         });
//         let mut event_manager = NopEventManager::new();
//
//         let feedback = ConstFeedback::new(false);
//         let objective = ConstFeedback::new(false);
//
//         let scheduler = RandScheduler::new();
//         let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);
//
//         let mut harness = |_buf: &BytesInput| ExitKind::Ok;
//         let mut executor = InProcessExecutor::new(
//             &mut harness,
//             tuple_list!(),
//             &mut fuzzer,
//             &mut states,
//             &mut event_manager,
//         )
//         .unwrap();
//
//         let mutator = HavocScheduledMutator::new(tuple_list!(BitFlipMutator::new()));
//         let mut stages = tuple_list!(StdMutationalStage::new(mutator));
//
//         for i in 0..1000 {
//             fuzzer
//                 .fuzz_one(&mut stages, &mut executor, &mut states, &mut event_manager)
//                 .unwrap_or_else(|err| panic!("Error in iter {i}: {err:?}"));
//             if cfg!(miri) {
//                 break;
//             }
//         }
//
//         let state_serialized = postcard::to_allocvec(&states).unwrap();
//         let state_deserialized: StdState<
//             InMemoryCorpus<BytesInput>,
//             _,
//             StdRand,
//             InMemoryCorpus<BytesInput>,
//         > = postcard::from_bytes::<
//             StdState<
//                 InMemoryCorpus<BytesInput>,
//                 BytesInput,
//                 RomuDuoJrRand,
//                 InMemoryCorpus<BytesInput>,
//             >,
//         >(state_serialized.as_slice())
//         .unwrap();
//         assert_eq!(state.corpus().count(), state_deserialized.corpus().count());
//
//         let corpus_serialized = postcard::to_allocvec(states.corpus()).unwrap();
//         let corpus_deserialized: InMemoryCorpus<BytesInput> =
//             postcard::from_bytes(corpus_serialized.as_slice()).unwrap();
//         assert_eq!(state.corpus().count(), corpus_deserialized.count());
//     }
// }
