# In-process

`In-process` fuzzer is a type of fuzzer where the fuzzer and the fuzzed target lives in the same process.
The target code and the fuzzer code are compiled together into a single binary.
The fuzzed target will expose an entry point for the fuzzer, and the fuzzer will mutate the input and execute the target by making a function call into the entry point.

# Typical setup
Typical setup for an inprocess-fuzzer will go like this.
First the user will compile their target into a object or archive file.
Do not forget to instrument your target with any instrumentation engine.

Similarly, the fuzzer should be also compiled into an archive file.
In `LibAFL--` or in Rust in general, this is done by specifying
```toml
[lib]
crate-type = ["staticlib"]
```

Lastly you can compile them together. 
The fuzzer will enter `main()` from the code of the fuzzer runtime. 
The runtime will call into the target that lives in the same binary and same process and do fuzzing.