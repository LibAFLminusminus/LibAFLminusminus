# Metadata

A metadata in `LibAFL--` is a self-contained structure that holds associated data to the State.

In terms of code, a metadata can be defined as a Rust struct registered in the SerdeAny register.

```rust
# extern crate libafl_bolts;
# extern crate serde;

use libafl_bolts::SerdeAny;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, SerdeAny)]
pub struct MyMetadata {
    //...
}
```

The struct must be static, so it cannot hold references to borrowed objects.

As an alternative to `derive(SerdeAny)` which is a proc-macro in `libafl_derive` the user can use `libafl_bolts::impl_serdeany!(MyMetadata);`.

## Why do we need metadata?
This concept of metadata serves as a method to cope with the problem of `in-process` execution mode. In `in-process` execution mode, a crash originating from the target also means the crash in the fuzzer process, because both the target and the fuzzer lives inside the same process. After the crash happened, we must, of course restart the fuzzer process and continue fuzzing. `Metadata` is a way to save and restore this metadata across restarts.

Another aspect of the metadata is that it works like a "global variable" that can bridge across different modules. Imagine that you observed the execution in `Observer/Feedback` but you want to use that data in another module `Scheduler`. For this, you can store the data into the metadata map of `State` and then retrieve it later. 

## Serialization and Deserialization

We are interested to store State's Metadata to not lose them in case of crash or stop of a fuzzer. To do that, they must be serialized and unserialized using Serde.

As Metadata is stored in a SerdeAnyMap as trait objects, they cannot be deserialized using Serde by default.

To cope with this problem, in `LibAFL--` each SerdeAny struct must be registered in a global registry that keeps track of types and allows the (de)serialization of the registered types.

Normally, the `impl_serdeany` macro does that for the user creating a constructor function that fills the registry, which will then be called automatically upon the start of the fuzzer.

## Usage

Metadata objects are primarly intended to be used inside [`SerdeAnyMap`](https://docs.rs/libafl_bolts/latest/libafl_bolts/serdeany/serdeany_registry/struct.SerdeAnyMap.html) and [`NamedSerdeAnyMap`](https://docs.rs/libafl_bolts/latest/libafl_bolts/serdeany/serdeany_registry/struct.NamedSerdeAnyMap.html).

With these metadata maps, the user can retrieve instances by type and name. Internally, the instances are stored as SerdeAny trait objects.

In `LibAFL--`, `StdState` holds an object of `NamedSerdeAnyMap`. Anything that you store inside this metadata maps is persistent across restarts. On the other hand, if you don't store your data inside this metadata map, your data will be lost across restarts. 

## Example
Let's take a simple example.
Conceptually, imagine that you want to save a "score" to represent the goodness of a testcase.

In this case, you can define a structure to hold all these score for the testcases so that they are not lost during restarts
```rust
pub struct ScoreMetadata {
    scores: HashMap<TestcaseId, f64>
}

libaflmm_bolts::impl_serdeany!(ScoreMetadata);
```

After, you can attach to this metadata to the state's metadata map.
```rust
let name = "example";
named_metadata_mut::<ScoreMetadata>(state.metadata_map_mut(), name)?;
```

## Named metadata and Unnamed metadata.
For historical reasons, we have two types of metadata map implemented; `SerdeAnyMap` and `NamedSerdeAnyMap`. However, in `LibAFL--`, we only use `NamedSerdeAnyMap` for simplicity. This means that the metadata map is indexed twice. It is first indexed by using the user-given `name`, and secondly indexed by using the type (type id) of the stored object. However, it is not always necessary that users want to use the `name` to index the name, and for this reason, we offer two sets of APIs. The "named" metadata APIs and the "unnamed" metadata APIs.

For example, `named_metadata_mut<'a, M>(map: &'a mut NamedSerdeAnyMap, name: &str)` finds the object that matches both the type `M` and the `name` from the `map`. Similarly, `unnamed_metadata_mut<M>(map: &mut NamedSerdeAnyMap)` finds the object that matches the type `M` and doesn't care about the name. Internally this API will just use the empty string `""` for matching against the name. If you don't care the names, you can simply use this "un-named" type of API.

# Dependency Resolver
So far, we've reviewed that in `LibAFL--` we use metadata map to keep data persistent across restarts. In reality, as your fuzzer gets more complex, one problem arises.

## Why dependency resolver?
Remember that we discussed that the metadata could be used as "global variables" that bridges different modules. So, for example, one module X can produce the data later to be used by another module Y. In this case, we have a dependency that the data consumed by module Y has a dependency on the data generated by module X. If, then, the user swapped module X with a similarly working module X' that doesn't produce the metadata, we'll have an error or a runtime panick in the fuzzer. This is a pain in that ass, and we can't easily resolve this problem with compile-time checkups, and thus, we bring the check at the startup.

We have a trait `DependencyResolver` to address this problem. This trait basically provides two APIs. Most modules in `LibAFL--` will implement this trait.
`register()` allows user to register any metadata that user use inside that specific module, and initialize it.
`check()` allows user to add arbitrary check to check if certain conditions are met with the registered metadata, for example, you can check if a specific metadata is registered already.

At the start-up of a fuzzer, we will first call `register()` to register and initialize all the metadata requested, then call `check()` to see if they are in a healthy state. 
