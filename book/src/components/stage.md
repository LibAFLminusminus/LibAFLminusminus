# Stage

A Stage is an entity that operates on a single Input received from the Corpus.
Stages are the bread of the butter of a fuzzer in a sense that they define what your fuzzer actually does to fuzz your target.

For instance, a `StdMutationalStage`, given an input of the corpus, applies a Mutator and executes the generated input for a couple of times. How many times this has to be done can be scheduled, AFL for instance uses a performance score of the input to choose how many times the havoc mutator should be invoked. This can depend also on other parameters, for instance, the length of the input if we want to just apply a sequential bitflip, or a fixed value.

Also, stages can be used to retrieve informations by running the un-mutated input just for once. 
For example, `TracerStage` will run the input with the given `tracer_executor`. 
This is mostly used for implementing `CmpLog` mutator.
 `TracerStage` combined with the provided tracer executor will execute the target once and collect all the operands used in cmplog instructions.

Other stages include:
`SingleRunStage`, is used for running the unmutated testcase once alongwith user-supplied hooks, and is useful for in-process cmplog observation.
`PowerScheduleStage`, is an elaborated version of `StdMutationalStage` biased toward "better" testcases (They do power schedules in fuzzing terms).
`GenStage`, will generate testcases with a `Generator` instead of mutating a testcase from the `Corpus`.

The decision to whether your fuzzer should execute a stage or not can be configured dynamically. 
We provide `IfStage`, `IfElseStage`, and `WhileStage`. 
For example, with `IfStage`, the fuzzer will run the stage only if the closure you provided evaluates to be true.
In a sense, you can "program" how you want to run the stages.

# Component relationship

We obviously want to run multiple stages, stages are often grouped inside a [tuple_list!](./concepts/tuples.md).
This tuple list is held by a [Fuzzer](./components/fuzzer.md) object.