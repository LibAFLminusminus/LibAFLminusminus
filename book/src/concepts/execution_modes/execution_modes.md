# Execution modes

`LibAFL--` can be used mainly through two main modes of execution: `Forkserver` and `In-process`.

**Forkserver** is the default and less error-prone execution mode, at the price of worse performance.
It is the default and recommended mode for easy fuzzing.
Its architecture is split into two main parts: the fuzzer and the forkserver.
The fuzzer contains the fuzzer state, sends inputs to the forkserver and receives feedback.
The forkserver, compiled alongside the target, receives the input, forks, provides the input to the target, then sends back the feedback to the fuzzer.

**In-process** is architecturally simpler: it keeps the fuzzer and the target in a single process.
In enables much better performance, but comes with more subtle issues the user may have to ultimately face.

The next few chapters will dive more deeply into how each execution mode works, their trade-offs, and how exactly they are integrated into `LibAFL--`.
