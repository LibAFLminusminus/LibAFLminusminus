# Corpus Sharing

Corpus sharing is a more advanced topic.
It can take multiple forms, and using the right kind of corpus sharing reveals to be subtle and often wrong.

## Main Idea

Corpus sharing is anything related to sharing test cases between workers.
Even through most fuzzers tend to propose a single way to share a corpus, strategies can in fact be extremely diverse, depending on multiple factors: worker configuration, time between synchronizations, order of synchronized inputs, etc...

In our definition, corpus sharing is not limited to "sharing inputs between workers to increase coverage".
Instead, we mean by there any form of input exchange between workers.
Think of a scenario in which you run some fuzzing workers, and would like to send all the newly found inputs in another client that runs a sanitizer to check for memory corruption bugs.
We consider this a form of corpus sharing.

## Disclaimer

Latest academic work does not conclude whether corpus sharing is always beneficial to the overall coverage.
In other words, running `N` fuzzers in parallel without any input shared tends to be better than well-known corpus sharing strategies (like the AFL++ `1 to N` topology).
It does not necessarily mean corpus sharing is useless, but it also means it's not trivial to conclude it will be useful if you goal is to maximize coverage.
It is also highly likely the effectiveness of corpus sharing will vary depending on the target, the fuzzer configuration, etc...

This is the main reason why we let the possibility to fully configure corpus sharing in `LibAFL--`.
We do not know what will work better for you, so you are responsible for finding the corpus sharing settings that work best for you.

## Groups

If `Worker` is the unit of execution, `Group` is the set of units.
In `LibAFL--`, workers are part of a `Group`.
A group is as simple as it sounds: it's a set of workers with the same configuration, which will all run on a specified cores.
The number of cores (pinned or not) is the number of workers in the groups.
All workers of a given groups share the same configuration (`task`).

# Synchronization Architecture

To synchronize inputs (and information in general) between workers, we use the composition of multiple traits.

## Commands and Notifications

These are the information exchanged between `Worker`s and the `Controller`:
- `Command` is what goes from the `Controller` to the `Worker`s (it "commands" the worker to do something)
- `Notification` is what goes from a `Worker` to the `Controller` (the worker "notifies" the controller of something)

This is what is basically sent over the wire.

## Exchange

An exchange is basically the protocol, which consists of a pair of `Command` / `Notification`.

It also defines how received notifications get translated back into commands.
Think of a notification for a newly found input received by the controller.
It must then be turned into a proper command that will be sent back to all the workers that should receive it.

The decider for "who receives from who" is decided by the `Router`, described below.

## Transfer

A transfer is basically defining the medium over which commands and notifications gets sent over.
This is the low-level mechanism that concretely makes the transfer happen.
It could take various forms, like sockets, pipes, shared memory, or whatever can be used to exchange information between workers and the controller.

## Routing

We call routing the algorithm that will decide which worker should send information to who.

This is where the concept of group comes in handy: the usual way to define routing is through the `GraphRouter`: you can simply define groups with an associated identity, and create unidirectional or bidirectional edges between them.

The easiest way to see it in action is with the `baby_corpus_sharing` example, which demonstrates how this router can be used in a simple setting.

## Orchestration

An `Orchestrator` is a "super type" that will regroup all the previously described objects

This is usually what the user will build first and set when selecting a corpus sharing strategy.

In practice, most orchestrators are simply concrete types over `GenericOrchestrator`, which is ready to be set for you use.

Check out `StdOrchestrator` or `GraphOrchestrator` for concrete examples.

# Standard Corpus Sharing

As explained above, we it is currently unclear is corpus sharing is even useful, or in which configuration it would actually work
.

Thus, we decided to keep as the `StdOrchestrator` a non-sharing orchestrator: no input get ever sent over between workers.

We will keep things that way until new work can properly show corpus sharing can improve coverage, and in which condition.
