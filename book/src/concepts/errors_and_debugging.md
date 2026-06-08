# Errors
In `LibAFL--` we define a class of errors in `libafl_core` crate.
You can use `libafl_core::Result<T>` (which is an alias of `Result<T, libafl_core::Error>`) for error handling. 

# Debugging
In this section, we accumulated common debugging tips you can use to debug your fuzzer made with LibAFL--.

## Q. My fuzzer crashed but the stack trace is useless (with all the call stacks pointing to Rust runtime.)

You can enable the `errors_backtrace` feature of the `libafl--` crate. With this the stacktrace is more helpful, pinpointing the place the error was raised at.

## Q. I started the fuzzer but the corpus count is 0

Unless the initial corpus is loaded with the `load_initial_inputs_forced` function, we only store the interesting inputs, which is the inputs that triggered the feedback. So this usually means that your input was not interesting, the feedback is set wrong, or your target was simply *not properly instrumented*.

Either way, what you can do is attach to the executable with gdb and set a breakpoint at where the new edges (or feedback otherwise) should be reported. If no instrumentation code is executed, then the problem is in the instrumentation. If the instrumentation code is hit, but still your input is not deemed interesting/stored, then the problem could be that you are not passing the observer/feedback correctly to the fuzzer.

## Q. I started the fuzzer but the coverage is 0

Essentially, this implies the same problem as the last one. Perhaps your target was not properly instrumented, or you are not using the correct observer, feedback feature.
In this case, again, what usually should do is to run the fuzzer with gdb and set a breakpoint at where the coverage is recorded (e.g. `__sanitizer_coverage_trace_pcguard`), and validate that the target is giving the feedback to the fuzzer.

## Q. I don't see any output from my fuzzer (println!() or logging)

First, check that you are not redirecting things to `/dev/null` else you will see nothing.
To see the log that you added with `log::trace!();`, you need to initialize the logger (any logger, `env_logger` or `SimpleStdoutLogger` from `libafl_bolts`) before the fuzzer starts.
Also you have to make sure that you are running with `RUST_LOG=<log_level>` and you are *NOT* using `release_max_level_info` feature of `log` crate in your `Cargo.toml` of your fuzzer

## Q. I still have problems with my fuzzer

Finally, if you really have no idea what is going on, run your fuzzer with logging enabled. (You can use `env_logger`) (Don't forget to enable stdout and stderr), and you can open an issue or ask us in Zulip (https://fuzz.zulipchat.com/) and find @toka or @rmalmain