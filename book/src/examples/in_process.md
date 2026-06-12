# In-process

This section will explain you the basics of in-process fuzzing with `LibAFL--` using an C target.
We will work on `fuzzers/baby/tutorial`

First, let's see how to get it running.

## Run

First you run `cargo build --release`. 
This will build two things, the fuzzer as a static library and the compiler wrapper.

Next, you run `compile.sh`. 
This will compile the target code and link it to the fuzzer static library to make the final fuzzer executable.

After that you can run `./fuzzer`.

## The compiler wrapper

`src/bin/libafl_cc.rs` and `src/bin/libafl_cxx.rs` work as the compiler wrapper for instrumenting the target.
No matter that your target is, you have to create a compiler wrapper if you want compile-timed instrumentation.

```rust
        let mut cc = ClangWrapper::new();
        if let Some(code) = cc
            .cpp(is_cpp)
            .silence(true)
            .parse_args(&args)
            .expect("Failed to parse the command line")
            .link_staticlib(&dir, "tutorial")
            .add_arg("-fsanitize-coverage=trace-pc-guard")
            .run()
            .expect("Failed to run the wrapped compiler")
        {
            std::process::exit(code);
        }
```
The important part about this compiler wrapper is the call to `link_staticlib`
This will tell the compiler to always link to `libtutorial.a`.
Thanks to this, once you build your compiler pass and the fuzzer (as a static library), you can link any object file to the fuzzer runtime code with the compiler pass.

`add_arg("-fsanitize-coverage=trace-pc-guard")` tells the compiler to instrument with sancov tracing PCs.
If you are interested, you can check [this](https://clang.llvm.org/docs/SanitizerCoverage.html).

All in all, the important thing to remember is that the compiler wrapper will automatically link the fuzzer code to the target.

## The input.