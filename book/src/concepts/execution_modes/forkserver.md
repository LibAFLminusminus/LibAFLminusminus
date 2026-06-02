# Forkserver

`Forkserver` is separated into two main processes:

- The **Fuzzer**: it is kept alive during the whole session, and takes care of "orchestrating" fuzzing.
It contains the fuzzer instance (which mutates the input, processes the feedback, etc...) and sends inputs to the *forkserver process*.
