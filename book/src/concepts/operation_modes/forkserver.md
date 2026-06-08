# Forkserver

`Forkserver` is separated into two main processes:

- The **Fuzzer**: it is kept alive during the whole session, and takes care of "orchestrating" fuzzing.
It contains the fuzzer instance (which mutates the input, processes the feedback, etc...) and sends inputs to the *forkserver process*.
- The **Forkserver**: it is compiled and linked with the target, and has two main roles: bridge the fuzzer with the target, and restart the target between executions.
As the forkserver's name suggests, it forks itself between each target execution, allowing for a clean state reset.

{{#drawio path="assets/forkserver.drawio" page=0}}

## The fuzzer

The fuzzer contains most of the `LibAFL--` logic.
It includes (but is not bounded to): the launcher (notably for multi-core support), the fuzzer, the mutators, the feedbacks, the persistent state, etc...

## The forkserver

The forkserver links the fuzzer and the actual target code.
