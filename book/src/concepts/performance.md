# Performance

## Analysis
`LibAFL--` offers a way for analyzing the performance of your fuzzer.
We collect the duration of time spent in each `Stage` in `PerfStats` structure.
You can write your own `Monitor` or use `WebMonitor` to view this information.

## Improving the performance
Improving the performance of your fuzzer is always a point of interest. 
This topic is not limited to `LibAFL--`, so we will discuss several general tips.

### Execution mode.
In-process execution mode is almost always more good-performing than the forkserver execution model. 
Obviously fork()-ing and execve()-ing into a new process at every execution is a heavy cost to pay for the fuzzer. 
If you can adapt your fuzz target into a in-process fuzzer, then it is always good for the performance, but it will also comes with the downsides like reduced stability.

### Binding to specific CPUs.
By binding your workload to a specific CPU with `sched_setaffinity` or just using the `taskset` frontend will speed-up things a bit. 
In `LibAFL--`, we can do this for you when you use `Launcher` and `cores()` to fix the cores.

### Timer
Fuzzers need to set and unset timers before and after executing the target, else the fuzzer will get stuck if the target decides to loop forever. 
Of course, those syscalls for timers is not free. 
If you don't need precise timer, and you just want something to prevent infinite loops, you can use `FastTimer` so that setting timers involve less syscalls.