# Input

Formally, the input of a program is the data taken from external sources that affect the program behavior.

In our model of an abstract fuzzer, we define the `Input` as the internal representation of the program input (or a part of it).

In the straightforward case, the input of the program is a byte array and we store and manipulate exactly these byte arrays during mutational fuzzing.

However, it is not always the case. 
Your `Input` does not necessarily have to be a byte array.
A program can expect inputs that are not linear byte arrays (e.g. a sequence of syscalls forming a use case or protocol) and the fuzzer does not represent the `Input` in the same way that the program consumes it.

In case of a grammar fuzzer for instance, the `Input` is generally an Abstract Syntax Tree because it is a data structure that can be easily manipulated while maintaining the validity

# Input Context

When the input can take any form other than a plain byte arrays, one thing that we have to consider is how to pass this `Input` to the program.
Since the program (usually) expects the input to be a byte array, we need a way to serialize from your `Input` into a byte array.

To this end, we provides a trait `InputContext`.
This trait defines a method `to_bytes` which you can implement in order to tell the fuzzer how to serialize the original `Input` down to the byte arrays.

# Component relationship

Each input is stored as a `Testcase` in `Corpus`. (`Corpus` is stored in [`State`](./state.md)).
Each state is associated with only one type of `Input` and [`State`](./state.md) also holds an object of `InputContext`.