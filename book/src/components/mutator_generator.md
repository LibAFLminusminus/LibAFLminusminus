# Mutator

The Mutator is an entity that takes one or more Inputs and generates a new instance of Input derived by its inputs.

Mutators can be composed, and they are generally linked to a specific Input type.

There can be, for instance, a Mutator that applies more than a single type of mutation to the input. Consider a generic Mutator for a byte stream, bit flip is just one of the possible mutations but not the only one, there is also, for instance, the random replacement of a byte of the copy of a chunk.

There are also mutators that always produce valid inputs, say a mutator that generates valid JSON or code, but these grammar based mutators need a grammar to work.

In LibAFL, [`Mutator`](https://docs.rs/libafl/latest/libafl/mutators/trait.Mutator.html) is a trait.

# Generator

A Generator is a component designed to generate an Input from scratch.

Typically, a random generator is used to generate random inputs.

Generators are traditionally less used in Feedback-driven Fuzzing, but there are exceptions, like Nautilus, that uses a Grammar generator to create the initial corpus and a sub-tree Generator as a mutation of its grammar Mutator.

In the code, [`Generator`](https://docs.rs/libafl/latest/libafl/generators/trait.Generator.html) is a trait.
