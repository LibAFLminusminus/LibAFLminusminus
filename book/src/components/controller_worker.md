# Controller & Worker

To ease communication between workers and collect information about a run, `LibAFL--` introduces the `Controller` and `Worker` traits.
While implementors of `Worker` represent a single worker running a fuzzer, a `Controller` implementor is the entity orchestrating all these workers.

A `Controller` and `Worker`s work in a 1-to-N architecture: a fuzzing run (launched with `StdLauncher` at least) will always have 1 controller, linked to N workers.
`LibAFL--` exposes multiple mechanisms to make them communicate altogether, and possible share inputs.
Check [`the synchronization documentation`](sync.md) for more information about that.

# Worker

A worker contains any information about the worker itself: its working directory, group and a unique identifier.
The fuzzer can use it to know where on-disk corpuses should be stored, or on which file descriptor `stdout` and `stderr` should be redirected.
It basically represents the bridge between a single fuzzing instance, and the more global fuzzing run.

# Controller

On the other side, a `Controller` spawns as many `Worker`s as necessary, and makes sure they are correctly configured.
A controller usually have some form of connection (like a socket) with every other worker.
It can be used to transmit inputs, or gracefully ask them to shut down.
