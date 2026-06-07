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

pub use libaflmm_core::{Error, Result};
pub use libaflmm_core::{
    empty, empty_optional, illegal_argument, illegal_state, internal_bug, invalid_corpus,
    invalid_input, iterator_end, key_exists, key_not_found, last_os_error, non_zero,
    non_zero_const, non_zero_unchecked, nonnull_raw_mut, not_implemented, os_error, runtime,
    unknown, unsupported,
};

pub mod common;
pub mod controllers;
pub mod corpus;
pub mod executors;
pub mod feedbacks;
pub mod fuzzers;
pub mod generators;
pub mod inputs;
pub mod launchers;
pub mod monitors;
pub mod mutators;
pub mod observers;
pub mod runtimes;
pub mod stages;
pub mod states;

/// The purpose of this module is to alleviate imports of many components by adding a glob import.
pub mod prelude {
    pub use libaflmm_core::{
        empty, empty_optional, illegal_argument, illegal_state, internal_bug, invalid_corpus,
        invalid_input, iterator_end, key_exists, key_not_found, last_os_error, non_zero,
        non_zero_const, non_zero_unchecked, nonnull_raw_mut, not_implemented, os_error, runtime,
        unknown, unsupported,
    };

    pub use libaflmm_bolts::{
        current_milliseconds, current_nanos, current_time, rands::StdRand, tuples::tuple_list,
    };

    pub use crate::{feedback_and, feedback_and_fast, feedback_not, feedback_or, feedback_or_fast};

    pub use crate::common::{CompatibilityChecker, DependencyResolver, Registrator};

    #[cfg(feature = "nautilus")]
    pub use crate::common::{
        ChunkStore, ChunkStoreWrapper, Context, GrammarMutator, NTermId, NodeId, PlainRule,
        RecursionInfo, RegExpRule, RegexScript, Rule, RuleChild, RuleId, RuleIdOrCustom, Tree,
        TreeLike, TreeMutation,
    };

    pub use crate::controllers::{
        Controller, Descriptor, NopController, NopDescriptor, NopWorker, SimpleController,
        SimpleDescriptor, SimpleWorker, StdController, StdDescriptor, StdWorker, Workdir,
        WorkdirFile, Worker,
    };

    pub use crate::corpus::{
        Cache, CachedOnDiskCorpus, CombinedCorpus, Corpus, DisableEntry, EnableDisableCorpus,
        FifoCache, IdentityCache, InMemoryCorpus, InMemoryOnDiskCorpus, InMemoryStore, NopCorpus,
        NopScheduler, OnDiskCorpus, OnDiskStore, QueueScheduler, RandScheduler, RemovableScheduler,
        Scheduler, SingleCorpus, StdCorpus, StdInMemoryCorpusMap, StdInMemoryStore,
        StdObjectiveCorpus, StdOnDiskStore, StdScheduler, Store, Testcase, TestcaseFilenameFormat,
        TestcaseId,
    };

    pub use crate::executors::{
        DiffExitKind, Executor, ExecutorsTuple, ExitKind, ForkserverChannel, ForkserverExecutor,
        NopExecutor, StdChildArgs, StdExecutor, common_signals,
    };

    pub use crate::feedbacks::{
        AflMapFeedback, AlwaysInterestingMapFeedback, BoolValueFeedback, ConstFeedback,
        CrashFeedback, DiffExitKindFeedback, EagerAndFeedback, EagerOrFeedback, ExitKindFeedback,
        Feedback, FeedbackFactory, ListFeedback, MapFeedback, MaxMapFeedback,
        MaxMapOneOrFilledFeedback, MaxMapPow2Feedback, MinMapFeedback, NewHashFeedback,
        NotFeedback, StdFeedback, StdMapFeedback, StdObjectiveFeedback, TimeFeedback,
        TimeoutFeedback,
    };

    #[cfg(feature = "nautilus")]
    pub use crate::feedbacks::NautilusFeedback;

    pub use crate::fuzzers::{
        CalibrationHook, CustomNameHook, Evaluator, ExecutionProcessor, Fuzzer, FuzzerHook,
        FuzzerHooksTuple, NopFuzzer, StdFuzzer,
    };

    pub use crate::generators::{Generator, RandBytesGenerator, RandPrintablesGenerator};

    #[cfg(feature = "nautilus")]
    pub use crate::generators::{NautilusContext, NautilusGenerator};

    pub use crate::inputs::{
        BytesContext, BytesInput, BytesSubInput, HasMutatorBytes, Input, InputContext, NopContext,
        NopInput, ResizableMutator, StdContext, StdInput, ValueInput,
    };

    #[cfg(feature = "nautilus")]
    pub use crate::inputs::NautilusInput;

    pub use crate::launchers::{
        DEFAULT_MAX_STATE_SIZE_PER_WORKER, Instance, InstanceId, Instances, StdLauncher,
    };

    pub use crate::monitors::{Monitor, PerfStats, SimpleMonitor, StdMonitor};

    #[cfg(feature = "web_monitor")]
    pub use crate::monitors::WebMonitor;

    pub use crate::mutators::{
        ComposedByMutations, HavocScheduledMutator, Mutator, MutatorsTuple, NopMutator,
        ScheduledMutator, StdMutator, Tokens, havoc_mutations, havoc_mutations_no_crossover,
        int_mutators, tokens_mutations,
    };

    #[cfg(feature = "nautilus")]
    pub use crate::mutators::{
        NautilusRandomMutator, NautilusRecursionMutator, NautilusSpliceMutator,
    };

    pub use crate::observers::{
        CmpLogMetadata, CmpLogObserver, ConstLenMapObserver, ConstMapObserver,
        HitcountsIterableMapObserver, HitcountsMapObserver, ListObserver, MapObserver,
        MultiMapObserver, Observer, ObserverWithHashField, ObserversTuple, OutputObserver,
        StdCmpLogObserver, StdErrObserver, StdMapObserver, StdObserver, StdOutObserver,
        TimeObserver, ValueObserver, VarLenMapObserver, VariableMapObserver,
    };

    pub use crate::runtimes::{
        InProcessRuntime, NopRuntime, RestartingRuntime, Runtime, RuntimeHandle,
        SimpleInProcessRuntime, SimpleRuntime, StdForkserverRuntime, StdInProcessRuntime,
    };

    pub use crate::stages::{
        DynamicStage, GenStage, IfElseStage, IfStage, MutationalStage, NopStage,
        PowerScheduleStage, RunHookFn, SingleRunStage, Stage, StagesTuple, StdMutationalStage,
        StdStage, TracerStage, WhileStage,
    };

    pub use crate::states::{NopState, State, StdState};
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
