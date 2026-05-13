# `LibAFL_derive`: Derive Macros for `LibAFLmm`

The `libaflmm_derive` crate offers derive macros, such as `#[derive(SerdeAny)]`.

## Available Derive Macros

### `#[derive(SerdeAny)]`

This macro implements the `SerdeAny` trait for a type. This is necessary to store the type in a `SerdeAnyMap`, a key component for type-safe storage of different data types in `LibAFL`.

**Usage:**

```rust,ignore
use libafl_derive::SerdeAny;
use serde::{Serialize, Deserialize};

#[derive(SerdeAny, Serialize, Deserialize)]
struct MyStruct {
    // ...
}
```

### `#[derive(Display)]`

This macro implements the `core::fmt::Display` trait for a struct. It generates a `Display` implementation that concatenates the string representations of all fields, separated by spaces.

**Special Handling:**

* **`Option<T>`**: If the value is `Some(inner)`, the inner value is displayed. If it is `None`, nothing is displayed.
* **`Vec<T>`**: The elements of the vector are displayed, each separated by a space.

**Example:**

```rust
use libafl_derive::Display;
use std::fmt::Display;

#[derive(Display)]
struct MyStruct {
    foo: String,
    bar: Option<u32>,
    baz: Vec<i32>,
}

let instance = MyStruct {
    foo: "hello".to_string(),
    bar: Some(42),
    baz: vec![1, 2, 3],
};
// The following will print: " hello 42 1 2 3"
println!("{}", instance);
```

## The `LibAFLmm` Project

This crate is part of the [LibAFLmm project](https://github.com/LibAFLminusminus/LibAFLminusminus).

The [README](../../README.md) contains the list of maintainers and licensing information.
