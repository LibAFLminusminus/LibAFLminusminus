# LibAFL to LibAFL\-- plans

# Feature Consensus

## definitions

- keep: obvious.
- rewrite: base can be kept the same, but major parts must be changed or removed.
- full rewrite: everything must be removed and rewritten. but the module in itself makes sense architecturally.
- remove: obvious.
- externalize: can be kept, but should be moved out of libafl. no one in libafl would maintain it.
- merge: move from separate crate to 

## table

| module                                              | decision1 | decision2 | note                                                                               |
|-----------------------------------------------------|-----------|-----------|------------------------------------------------------------------------------------|
| build_id2                                           | keep      | keep      |                                                                                    |
| core_affinity2                                      | keep      | keep      |                                                                                    |
| exceptional                                         | keep      | merge     | i don't know if it needs to be split tbh. it can be in bolts.                                                                                   |
| fast_rands                                          | keep      | keep      |                                                                                    |
| libafl/common                                       | keep      | keep      |                                                                                    |
| corpus (P1)                                             | rewrite   | rewrite   | only keep inmem and ondisk. the only good point of cache is for very large corpus and ram is limited. is it a problem? not ordered! |
| events  (P1)                                            | remove    | full rewrite | only keep restarting, but needs to be implemented in another way. why not just use normal IPC? there are easy ways to do that with all OSes.                   |
| executors (P2)                                          | rewrite   | rewrite | only keep inproc + fork. make fork the default ("StdExecutor" = "StdForkExecutor"). signal processing should be fully rewritten, it's very very shit. too many generics |
| feedbacks                                           | keep      | keep      |                                                                                    |
| fuzzer (P1)                                          | rewrite   | rewrite   | the code is shit, too many useless shit. we need to split the big ass functions there. it is handling too much critial stuff for no reason.                                            |
| generator                                           | keep      | keep      |                                                                                    |
| inputs                                              | keep      | keep      |                                                                                    |
| monitors  (P3)                                          | rewrite   | rewrite   | remove TUI, and other useless shits. it would be nice to have a "statsmonitor" to dump fuzzer stats at runtime, like in json or smth.                                                |
| mutators  (P3)                                          | rewrite   | rewrite   | the code is shit too. i think it's not a priority for now, but yes. it's easy to fix later.                                                               |
| observers                                           | keep      | keep      |                                                                                    |
| schedulers  (P3)                                        | rewrite   | rewrite   | the code is shit. this shit is dependent on other modules too. rewrite queuesched.                      |
| stages  (P3)                                            | rewrite   | rewrite   | too many shits                                                                     |
| state   (P1)                                            | rewrite      | rewrite      | remove HasState*. just 1 struct                                                                                   |
| libafl_asan                                         | ?         | keep      | does it work?? yes, i tried for another project. but could be improved. and clippy is not enabled on this one, so enabling it will add more work.                                                                     |
| libafl_bolts                                        | keep      | keep      |                                                                                    |
| libafl_cc                                           | keep      | keep      |                                                                                    |
| libafl_concolic                                     | remove    | externalize | nobody uses it. this should be external to libafl i think. it's useless.                                                                     |
| libafl_derive                                       | keep      | keep      |                                                                                    |
| libafl_frida                                        | ?         | keep      | why frida? just use qemu. -> it's useful for on-device fuzzing. it's good when you cannot emulate and you want to fuzz on a phone directly. we should keep i think, it's actually used.                                                          |
| libafl_intelpt                                      | keep      | keep      |                                                                                    |
| libafl_libfuzzer/libafl_libfuzzer_runtime           | remove    | remove | imo remove, it's broken and addison is not fixing shit. 100% agree. it breaks all the time and the code is unreadable / unmaintainable. i think we can do the same but in much simpler, like as a normal fuzzer with symbol linking.                             |
| libafl_nyx                                          | keep      | keep      | this one is working (surprisingly). keep for me                                                 |
| libafl_sugar                                        | remove    | remove    | bullshit. just copy paste fuzzers.                                                                           |
| libafl_targets (P3)                                    | rewrite   | rewrite   | it's shit but we need it so need rewrite. i think the problem there is the way it fits architecturally.                                           |
| libafl_tinyinst                                    | remove         | remove | idk imo nobody is using but the maintainance cost is low so i'm fine maintining it |
| libafl_unicorn                                      | remove    | remove | why unicorn? imo not worth including it                                            |
| ll_mp                                               | remove    | remove lol  | removeremoveremoveremove                                                           |
| minibsod                                            | keep      | rewrite   | there are some issues like super big stack traces with qemu, can be improved.                                                                                   |
| no_std_time                                         |           | ?         | no, we don't care no_std; just emulate with qemu.                                  |
| nonzero_macros                                      | keep      | keep      |                                                                                    |
| utils/build_and_test_fuzzers                        |           | keep      |                                                                                    |
| utils/cfg_builder                                   |           | keep      |                                                                                    |
| utils/ci_runner                                     |           | keep      |                                                                                    |
| utils/ci_splitter                                   |           | keep      |                                                                                    |
| utils/deexit                                        |           | ?         |                                                                                    |
| utils/drcov_utils                                   |           | keep      |                                                                                    |
| utils/find_llvm_config                              |           | keep      |                                                                                    |
| utils/gdb_qemu                                      |           | ?         | never used it                                                                      |
| utils/gramatron                                     |           | externalize | should not be maintained by libafl i think                                        |
| utils/libafl_benches                                |           | keep      |                                                                                    |
| utils/libafl_jumper                                 |           | remove?   | useless?                                                                            |
| utils/libafl_repo_tools                             |           | keep      |                                                                                    |
| utils/multi_machine_generator                       |           | ?         |                                                                                    |
| utils/noaslr                                        |           | keep?     |                                                                                    |
| others                                              | keep      | keep      |                                                                                    |


## Work queue

Romain:
- [ ] corpus

Toka:
- [ ] state
- [ ] fuzzer

# Structural problems

## Not everything can be seperated into modules
In some cases, modules need to communicate with each other. Currently we use a global-data for handling it (aka metadata). I think this is a good solution but relying on metadata is very hacky.
We should accept the need for global variables but design how to handle it in a better way

## There should be only one way to do a thing.
Each module should have clear definition on what it should do.
We should have a design that, 
when you want to implement a feature, you should be able to clearly tell where you're supposed to implement this stuff.
For example, too me, the distinction between observer/feedback is vague. fuzzer/stages too.

## State is not clearly defined
It's unclear what is getting saved and restored on crash.
Why should fuzzer contain feedback but not corpus for example?

# Mindset

## Respect CI
obvious. fix ci and merge. 
if ci is unfixable? why? if the people responsible for that code is not responding then that code should be removed. (e.g. libafl_libfuzzer)
if ci is failing for rust-side error? fix the version of the rust used in CI.
don't bring shitty nightly clp lints.
If CI does not work because a website is down? Do not merge and wait to rerun the ci fully.

## Move slow and fix things
not "Move fast and break things". There is always somebody fixing shit code for your mess. and no, i don't want to clean your shits anymore.
don't merge unstable things. don't be like "somebody will notice and will fix it"
We care about stability and usability. don't ship broken code.
It's ok to have bugs, if the code was considered ready to be merged and thought to be bug free in the first place.
It's not ok if random things get merged thinking "someone will figure it out later".

## Library should be kept simple
Imo libafl is too bloated, with so many un-maintained/broken features.
if a feature is too extensive, we should not accept PRs for that. 
why merge something that will break in a few years? If you want something, create your own repo.
A feature should be added if the person taking ownership of the feature can be clearly identified.
if noone can take care of it, there is no point adopting the feature in the first place.

## Draw a line between what we do and what we don't do
It's important to decide what you "DON'T" do in the library. it's more important than deciding what you "DO"
it's not like you merge every fancy features and break the library.
we should clearly draw a line between what we support and what we don't

## Do not be scared to unwrap
If something fails and would leads to unrecoverable / corrupted state, just unwrap.
Rust is bad with error propagation and it makes it hard to recover the root problem.
unwrap by default, and return an error if it makes sense to do something else when it happens.
unwrap are not a bad thing.

## Documentation
this is a unfinished job of andrea. There is no proper documentation of libafl after 4 years.
we should complete documentation while we are going through the codebase early.
there won't be a better time than now to do it, since we will have to go through everything.

## Platforms
I don't care about macos. will remove it. buy yourself a linux/windows computer.

## Performance

Adding a complex feature to improve speed is fine.
But only do it if you can actually show it has a real impact, or you are solving some bottleneck.
Do not merge a feature adding 10K LoC full of complex concurrent low-level code if it does not a have a clear benefit.

# About events

Romain: should we really ditch the concept completely? i think it still makes sense to have some kind of lightweight exchange mechanism, at least to share stats? i think the underlying question is: should fuzzer instances be completely independant? i feel like sharing things could still be useful, but in a simple way.
Or we go back to afl++ approach: one client per folder, and use 1 file per instance to store stuff. The good point of shared memory is that it makes it easy to share other things in the future, it's more flexible.
anyway, i'd to like afl++: one master node, N-1 slave nodes, and the master collects / shares or whatever. no broker, no third party, just fuzzers.

if we go for something else, we need to define exactly how we will replace each event:

| event           |replacement |
|-----------------|------------|
| NewTestcase     |  use FS    |
| Heartbeat       |  useless?  |
| UpdateUserStats |            |
| Objective       |  use FS    |
| Log             |            |
| Stop            |            |


# About state

We need to define exactly what state is and how state should be accessed.
for now, we have:



```
// !!!! remove metadata from testcase

// will be checked by fuzzer before start
pub unsafe trait DeclareMetadataTypes {
    // create a default global metadata 
    pub fn declare<M>(&mut self, metadata: M);

    // create a default per-testcase metadata when a testcase is added
    pub fn declare_per_testcase<M>(&mut self, metadata: M);
}

pub trait Mutator: DeclareMetadataTypes {}

pub struct MetadataChecker {
    types: HashSet<TypeId>;
}

impl MetadataChecker {
    pub fn register(&mut self) {}
    pub fn check(&self, state: &mut S) {}
}

pub struct MetadataMap {
    map: HashMap<TypeRepr, HashMap<String, Box<dyn crate::serdeany::SerdeAny>>>,
}

impl MetadataMap {
    // use default name
    pub fn add();
    
    pub fn add_named();
}

pub State<CS> {
    rand,
    corpus,
    ...
    metadata: Metadata,
    testcase_metadata: HashMap<CorpusId, Metadata>,
}


impl State<CS> {
    pub fn metadata(&mut self, name) -> Metadata {
    
    }

}

macro: declare_module_hook!(trait, ...) -> create hook pre/post for trait.

pub trait Hook {
    fn pre_exec(&mut self, state: &mut S) {}
    fn post_exec(&mut self, state: &mut S) {}
}

```

1. State:
    - rand
    - stats
    - corpus
    - solutions
    - metadata
    - current corpus id
    - scheduler <--- moved from fuzzer
    - workdir: PathBuf

2. Fuzzer:
    - feedback <- decision maker, no state out of metadata
    - objective feedback

3. Executor:
    - ObserverHook
    - observer <- keep "logs"

Observer HitCount -> find a way to fit in the state, think about it.


# Corpus vs scheduler

for now, corpus has an order used by scheduler implicitely.
imho, we should have
- Corpus: an unordered map of CorpusId -> Testcase
- Scheduler: provides the next CorpusId. ordering should be done by the scheduler.


# Notes

- merge restarting with inprocess executor

# general tasks

- remove useless traits
- documentation
- remove nostd
