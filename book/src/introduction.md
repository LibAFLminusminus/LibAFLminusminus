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

To tackle this, `LibAFL` was built as a collection of reusable pieces for individual fuzzers.
Written in Rust, it helped develop fuzzers tailored for specific needs.

Over time, `LibAFL` accumulated a broad set of features in a single codebase.
Many modules ended up rarely used in practice, and the resulting surface area became difficult to maintain.
We also identified core architectural limitations that called for rewriting significant parts of the code.
This is how the idea of `LibAFL--` was born.

## Why `LibAFL--`?

`LibAFL--` keeps the main benefits of `LibAFL`, while exposing a saner API for users.
Some highlight features currently include:

- **Consistent API**: In `LibAFL`, the way a fuzzer was assembled varied between use cases.
Some required a launcher, others did not.
`LibAFL--` unifies these into one single general shape.
That way, fuzzers can be easily modified and extended with minimal efforts.
- **Clear defaults**:
`LibAFL--` provides a default for every main block, through the `Std...` naming convention.
The standard set of mutators, stages, and other components is documented and usable out of the box.
- **Adaptable**: `LibAFL` offered fair adaptability, but some architectural decisions resulted in an awkward API for end user.
`Executor`s, for example, carried more responsibilities than they needed to, and have been rebuilt from scratch.
Other tasks had to be expressed through unrelated abstractions.
`LibAFL--` addresses this by exposing many more hooks.
- **Fast**: We do everything we can at compile time so that the runtime overhead is as minimal as it can get.
- **Bring your own target**: We support binary-only modes, like (full-system) QEMU-Mode and Frida-Mode with ASan and CmpLog, as well as multiple compilation passes for sourced-based instrumentation.
Of course, we also support custom instrumentation, as you can see in the Python example based on Google's Atheris.
- **Multi-platform**: LibAFL-- works on a more restrained set of platforms than LibAFL at the moment.
The main supported platforms are *Linux* and *Windows*.
