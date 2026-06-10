# Executor

In different fuzzers, this concept of executing the program under test does not always mean the same thing.
For instance, for in-process fuzzers like libFuzzer an execution is a call to an harness function, for hypervisor-based fuzzers like [kAFL](https://github.com/IntelLabs/kAFL) instead an entire operating system is started from a snapshot for each run.

In `LibAFL--`, an `Executor` is a trait that defines not only how to execute the target, but all the volatile operations that are related to just a single run of the target.

Executors represent instances responsible for informing the program about the input that the fuzzer wants to use in the run, writing to a memory location for instance or passing it as a parameter to the harness function, and executes it after.

In our model, it can also hold a set of Observers connected with each execution.

The two type of major executors we implemented is `InProcessExecutor` and `ForkserverExecutor`
In `InProcessExecutor` the target is a harness function, and the executor will take responsibility to pass inputs to the harness function and make a call into it.
Typically you will need compile-time instrumentation, such as sanitizer coverage instrumentation, to use this executor.
In `ForkserverExecutor`, we provide AFL/AFL++-like mechanism of forkserver to fuzz targets compiled with forkserver runtimes. 
In addition to the compile-time instrumentation, you will need forkserver instrumentation by AFL/AFL++ compilers.

We also have other execution engines using Qemu, Frida, Nyx, and they all provide corresponding executors, but we will cover on those topics in later chapters.

## InProcessExecutor

Let's begin with the base case; `InProcessExecutor`.
This executor executes the harness program (function) inside the fuzzer process.

When you want to execute the harness as fast as possible, you will most probably want to use this `InprocessExecutor`.

One thing to note here is, when your harness is likely to have heap corruption bugs, you want to use another allocator so that corrupted heap does not affect the fuzzer itself.
For example, we adopt `MiMalloc` in some of our fuzzers.
Alternatively you can compile your harness with address sanitizer to make sure you can catch these heap bugs.

## ForkserverExecutor

Next, we'll take a look at the `ForkserverExecutor`. In this case, it is `afl-cc` (from AFL/AFLplusplus) that compiles the harness code.
We need to setup the shared memory region to communicate between forkserver and the fuzzer.
For example,

```rust,ignore
    // Make a shared memory buffer.
    let mut shmem_buf = SysVShm::new(MAP_SIZE).unwrap();

    // Tell the forkserver about the shared memory location
    unsafe {
        shmem_buf.write_to_env("__AFL_SHM_ID").unwrap();
    }
```

Here we make a shared memory region; `shmem_buf`, and write this to environmental variable `__AFL_SHM_ID`. Then both the instrumented binary and the forkserver, finds this shared memory region (from the aforementioned env var) to record its coverage.
On your fuzzer side, you can pass this shmem map to your `Observer` to obtain coverage feedbacks combined with any `Feedback`.

# Component relationship

Executor is a component held by a [`Fuzzer`](./components/fuzzer.md)