# Sync

This module defines all the abstractions necessary to build any kind of synchronization between worker groups.

Please first take a look at [Corpus Sharing](../concepts/corpus_sharing.md) to get a higher-level overview of the ideas behind sync.

## `Group`s

If `Worker` is the unit of execution, `Group` is the set of units.
In `LibAFL--`, workers are part of a `Group`.
A group is as simple as it sounds: it's a set of workers with the same configuration, which will all run on a specified cores.
The number of cores (pinned or not) is the number of workers in the groups.
All workers of a given groups share the same configuration (`task`).

## `Command`s and `Notification`s

These are the information exchanged between `Worker`s and the `Controller`:
- `Command` is what goes from the `Controller` to the `Worker`s (it "commands" the worker to do something)
- `Notification` is what goes from a `Worker` to the `Controller` (the worker "notifies" the controller of something)

This is what is basically sent over the wire.

## `Exchange` trait

An exchange is basically the protocol, which consists of a pair of `Command` / `Notification`.

It also defines how received notifications get translated back into commands.
Think of a notification for a newly found input received by the controller.
It must then be turned into a proper command that will be sent back to all the workers that should receive it.

The decider for "who receives from who" is decided by the `Router`, described below.

## `Transfer` trait

A transfer is basically defining the medium over which commands and notifications gets sent over.
This is the low-level mechanism that concretely makes the transfer happen.
It could take various forms, like sockets, pipes, shared memory, or whatever can be used to exchange information between workers and the controller.

## `Router` trait

We call routing the algorithm that will decide which worker should send information to who.

This is where the concept of group comes in handy: the usual way to define routing is through the `GraphRouter`: you can simply define groups with an associated identity, and create unidirectional or bidirectional edges between them.

The easiest way to see it in action is with the `baby_corpus_sharing` example, which demonstrates how this router can be used in a simple setting.

## `Orchestrator` trait

An `Orchestrator` is a "super type" that will regroup all the previously described objects

This is usually what the user will build first and set when selecting a corpus sharing strategy.

In practice, most orchestrators are simply concrete types over `GenericOrchestrator`, which is ready to be set for you use.

Check out `StdOrchestrator` or `GraphOrchestrator` for concrete examples.
