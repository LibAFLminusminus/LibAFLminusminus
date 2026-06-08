# Mutator and Mutations

The `Mutator` is an entity that takes one or more `Input` and generates a new instance of `Input`.

Mutators can be composed, and they are generally linked to a specific Input type.

There can be, for instance, a Mutator that applies more than a single type of mutation to the input. 
We implement all this mutation operation in the `Mutation` trait in `LibAFL--`

In other words, `Mutation` is the component representing each mutation operation, and if you combine them together, then it becomes a `Mutator`

Consider a generic Mutator for a byte stream, bit flip is just one of the possible mutations but not the only one, there is also, for instance, the random replacement of a byte of the copy of a chunk.

There are also mutators that always produce valid inputs, say a mutator that generates valid JSON or code, but these grammar based mutators need a grammar to work.

# Generator

A `Generator` is a component designed to generate an Input from scratch.

Generators are traditionally less used in Feedback-driven Fuzzing, but there are exceptions, like Nautilus, that uses a Grammar generator to create the initial corpus and a sub-tree Generator as a mutation of its grammar Mutator.

# Component relationship

Both the `Mutator` and `Generator` are held by `Stages`. 
Typically, mutational stages will contain a `Mutator` and generational stages will contain a `Generator`
