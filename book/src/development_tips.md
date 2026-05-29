# Development Tips

We gather here some useful tips for developers to help create new fuzzers more easily.

## Use `prelude` to help with imports

We expose for each crate a `prelude` module in their root module.
If you are new to `LibAFL--`, this will help a lot to avoid searching for correct import paths and reduce the boilerplate code in general.
To use it, simply put in your import list:

```rust
use libaflmm::prelude::*;
```

This will import the most used types.
Some types are not exported as they are quite niche or would collide with existing widely used types.
In that case, you can always import types in the usual way.

## Error handling

`LibAFL--` exposes a single `Error` type, accessible via `libaflmm::Error`.
Similarly, we also expose our own `Result<T>` (returning `libaflmm::Error` as the error) through `libaflmm::Result`.
We encourage developers to stick to there errors for `LibAFL--`-related stuff to ease error propagation.
