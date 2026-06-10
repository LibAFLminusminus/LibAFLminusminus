# Feedback

The Feedback is an entity that classifies the outcome of an execution of the program under test as interesting or not.
Typically, if an execution is interesting, the corresponding input used to feed the target program is added to a corpus.

Most of the time, the notion of Feedback is deeply linked to the Observer, but they are different concepts.

The Feedback, in most of the cases, processes the information reported by one or more observers to decide if the execution is interesting.
The concept of "interestingness" is abstract, but typically it is related to a novelty search (i.e. interesting inputs are those that reach a previously unseen edge in the control flow graph).

As an example, given an Observer that reports all the sizes of memory allocations, a maximization Feedback can be used to maximize these sizes to sport pathological inputs in terms of memory consumption.

You can think of `Feedback` as a criteria for the fuzzer to make decisions.
The trait `Feedback` is used to define objects that, given the state of the observers from the last execution, tells if the execution was interesting.
Ultimately, it reduces the observations to a boolean result of `is interesting` or not.

Often, you want to store something to persist in the fuzzer's state when you use `Feedback`s.
This might be, for instance, the cumulative map of all edges seen so far, in the case of a feedback based on edge coverage.
This can be achieved by adding `Metadata`. We explain this concept in [another section](../concepts/metadata.md)
Typically you want to add those metadata in `append_metadata` method of `Feedback` trait.

Multiple Feedbacks can be combined into a boolean expression. 
For example, if you have two feedback A, B and only want to return true when both are true. 
In this case,
```rust
let feedback = feedback_and!(a, b);
```

We have `feedback_and!`, `feedback_or!`, and `feedback_not!` macros that provides exactly same functionality as the common logic operators.
For example,
```rust
let feedback = feedback_or!(a, feedback_and!(b, feedback_not!(c)));
```
will return true if 1. "a is true" or 2. "b is true and c is false".

On top, logic operators like `feedback_or` and `feedback_and` have a `_fast` variant (e.g. `feedback_or_fast`) where the second feedback will not be evaluated, if the value of the first feedback operand already answers the `interestingness` question so as to save precious performance.

Our collection of feedbacks includes:
`StdMapFeedback`: the standard map feedback for max-map coverage evaluation. It automatically explot SIMD instructions for speedups.
`BoolValueFeedback`: the feedback for evaluating a single boolean value.
`ListFeedback`: the feedback for evaluating novelties with a hashset.

## Objectives

While feedbacks are commonly used to decide if an [`Input`](./input.md) should be kept for future mutations, they serve a double-purpose, as so-called `Objective Feedbacks`.
In this case, the `interestingness` of a feedback indicates if an `Objective` has been hit.
Commonly, these objectives would be a crash or a timeout, but they can also be used to detect if specific parts of the program have been reached, for sanitization, or a differential fuzzing success.
Objectives use the same trait as a normal `Feedback` and the implementations can be used interchangeably.

## In relation to corpus

Any testcase that are deemed as `interesting` by `Feedback` will go to the `corpus` contained inside a `State`(./components/state.md).
On the other hand, any testcase found as `interesting` by `Objective Feedback` will go to the `objective corpus` inside a `State`.
The difference between these two `corpus` is that, the testcases in normal `corpus` will be picked up by the scheduler and mutated during the later fuzzing campaign.
The testcases in `objective corpus` won't be further mutated, it's a dead end. 
Usually they are the crashes and timeouts ready to be analyzed.

# Component relationship

Both `Feedback` and `Objective Feedback` is a object held by a [`Fuzzer`](./fuzzer.md) object.