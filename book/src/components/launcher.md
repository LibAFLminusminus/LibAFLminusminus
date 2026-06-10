# Launcher

Launcher is a module responsible for spawning and starting up the fuzzer instances and monitors them till they exit.
Before launcher creates spawns each fuzzer, it will create `Worker`(./components/worker.md) for each of the fuzzer instance.
The `Worker` is a instance that holds all the data specific to a fuzzing process, such as `pid`, which core it is bound to, the working directory, and so on.
We adopt a clear `Controller-Worker` model here. 
Each fuzzer process will have their own resource space described by each `Worker`, and all the `Worker`s are managed by the `Controller` to have a centralized view of what is happening across all the fuzzers. 
This way, each fuzzer instance will not interfere with each other.

From user's point of view, you have to know that launcher takes a `task` closure defining what the fuzzer wants will do. 
This is the main piece of code representing the fuzzer's behavior and you will mainly code this `task` closure to build your fuzzer.

## Monitor

After the process is launched, the launcher will use `Monitor` to monitor the fuzzers. 
Obviously this is a module to show the fuzzer's progress to the user.

For now, we have two modules.
`SimpleMonitor` for dumping basic stats to `stdout`.
`WebMonitor` for a UI that you can view inside your browser.


# Component relationship

Launcher is a top-level module for process management.
Launcher will contain a `Monitor` and a [`Controller`](./components/controller.md). Both objects are used for managing and monitoring multiple fuzzer processes involved.