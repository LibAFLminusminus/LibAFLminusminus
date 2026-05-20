# Introduction

Fuzzers are important tools for security researchers and developers alike.
A wide range of state-of-the-art tools like [AFL++](https://github.com/AFLplusplus/AFLplusplus), [libFuzzer](https://llvm.org/docs/LibFuzzer.html) or [honggfuzz](https://github.com/google/honggfuzz) are available to users.
They do their job in an effective way, finding thousands of bugs.

From the perspective of a power user, however, these tools are limited.
Their designs do not treat extensibility as a first-class citizen.
Usually, a fuzzer developer can choose to either fork one of these existing tools, or to create a new fuzzer from scratch.
In any case, researchers end up with tons of fuzzers, all of which are incompatible with each other.
Their outstanding features cannot just be combined for new projects.
By reinventing the wheel over and over, we may completely miss out on features that are complex to reimplement.

To tackle this, LibAFL was built as a collection of reusable pieces for individual fuzzers.
Written in Rust, it helped develop fuzzers tailored for specific needs.

However, LibAFL suffered from multiple problems that made it difficult to maintain over time.
The project included many different features that kept accumulating, resulting in a massive codebase with many modules few people were actually using.
Soon enough, LibAFL got heavily bloated, making it hardly maintainable.
We also identified core architectural issues, requiring to rewrite a large portion of the code.
This is how the idea of LibAFL-- was born.

## Why LibAFL--?

LibAFL-- keeps the main benefits of LibAFL, while exposing a saner API for users.
Some highlight features currently include:

- **Consistent API**: LibAFL was forcing users to change the way their fuzzers was built depending on the actual use case without any good reason.
For example, some fuzzers required a launcher, others did not. 
- **Clear defaults**: LibAFL-- provides a default for every main bloc, through the `Std...` naming convention.
You do not have to guess anymore what is the best general mutators to plug, or which stages should actually be used for everyday fuzzing.
- **Adaptable**: Although LiBAFL provided fair adaptability, architectural quirks made is awkward to perform some tasks.
For example, `Executor`s were uselessly complex and have been rebuilt from scratch.
Some anti-patterns were also necessary to perform specific tasks: for example, `Feedback` were sometimes used to hook into interesting test cases, even though it had nothing to do with a proper feedback.
LibAFL-- solves that by exposing many more hooks for the fuzzer and friends.
- **Fast**: We do everything we can at compile time so that the runtime overhead is as minimal as it can get.
- **Bring your own target**: We support binary-only modes, like (full-system) QEMU-Mode and Frida-Mode with ASan and CmpLog, as well as multiple compilation passes for sourced-based instrumentation.
Of course, we also support custom instrumentation, as you can see in the Python example based on Google's Atheris.
- **Multi-platform**: LibAFL-- works on a more restrained set of platforms than LibAFL at the moment.
The main supported platforms are *Linux* and *Windows*.
