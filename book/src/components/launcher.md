# Launcher

Launcher is a module responsible for spawning and starting up the fuzzer instances and monitors them until the fuzzers exit.
Before launcher spawns each fuzzer, it will create a [`Worker`](./controller_worker.md) for each of the fuzzer instance.
The `Worker` is a instance that holds all the data specific to a fuzzing process, such as its `pid`, which core it is bound to (if it is), the working directory, and so on.
To control the fuzzing run, we followed a `Controller-Worker` model: each fuzzer process has its own resource space described by each `Worker`, and all the `Worker`s are managed by the `Controller` to have a centralized view of what is happening across all fuzzer instances. 
This way, each fuzzer instance will not interfere with each other.

In addition, we also introduced the concept of `Group`s.
The idea is simple: each group owns its own configuration, being the [`Runtime`](./runtime.md) running the fuzzer, the timeout value, or even the cores it is owning.
This is the intended way to finely setup a fuzzing run with instances with different configurations.
For example, this allows to have some cores running sanitizers without being pinned on a specific core (since it is supposed to be run sporadically), while the main fuzzing instances can run on pinned cores with the highest performance.

## In practice

As the user, you only need to know that launcher own multiple `group`s defining what the fuzzer will do once launched. 
Each group is setup with its own `task`.
This is the highest level piece of code representing the fuzzer's behavior and the fuzzer codes mainly lives in the `task` closure.

Please refer to example fuzzers to get a clear idea of how these pieces plug together.

## Monitor

After the process is launched, the launcher will use a `Monitor` to monitor the fuzzing instances. 
Obviously, it is a module to show the fuzzer's progress to the user as it goes on.

For now, we have two main monitors:
- `SimpleMonitor` for dumping basic stats to `stdout`.
This is the simplest option available that provides a fair amount of information.
- `WebMonitor` for a web-based UI that can be reached from a browser.
It shows more interesting and various statistics in addition to what the `SimpleMonitor` provides.

# Relationship

Launcher is the top-level module for fuzzing instances management.
The launcher contains a `Monitor`, a [`Controller`](./controller_worker.md), and multiple `Group`s.
Each `Group` contains a fuzzer task and various configuration on how it will get spawned.
