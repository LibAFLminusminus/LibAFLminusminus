# Observer

An Observer is an entity that provides an information observed during the execution of the program under test to the fuzzer.

The information contained in the Observer is not preserved across executions, but it may be serialized and passed on to other nodes if an `Input` is considered `interesting`, and added to the `Corpus`.

As an example, the coverage map, filled during the execution to report the executed edges used by fuzzers can be considered an observation.
In `LibAFL--` this is implemented with `MapObserver` and its minor variants.

For example:
`StdMapObserver` is the most basic map observer.
`ConstMapObserver` is the map observer you can use when you know the map size at compile time.
`VariableMapObserver` allows you to adaptively change the map size at runtime.
`HitcountsMapObserver` will do the hit count bucketing from AFL. (Search for it if you want to more).

Another type of `Observer` can collect the time spent executing a run, the program output, or a more advanced observation, like maximum stack depth at runtime.

For example:
`CmpLogObserver` will observe the cmplog data from the instrumentation.
`OutputObserver` will observe `stdout` or `stderr` from the target.
`ValueObserver` will observe a value.

In short, `Observer` provides a peek into the dynamic property of the program.

In addition to holding the volatile data connected with the last execution of the target, the structures implementing this trait can define some execution hooks that are executed before and after each fuzz case. 
In these hooks, the observer can modify the fuzzer's state.
This is implemented as the `pre_exec` and `post_exec` API of the `Observer` trait.

The fuzzer will act based on these observers through a [`Feedback`](./feedback.md), that reduces the observation to the choice if a testcase is `interesting` for the fuzzer, or not.

# Component relationship

Observers are grouped-up in a [`tuple_list!`](../concepts/tuples.md).
This tuple list is then held by [Executors](./executor.md).