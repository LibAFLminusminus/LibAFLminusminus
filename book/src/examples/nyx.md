# Nyx

This section will explain `baby_nyx` example in `fuzzers/baby` directory.

## Building

First you have to build with `cargo build`. (not with `--release`!) This will build the fuzzer binary, and at the same time, it will download packer and QEMU-Nyx code into the target directory (`target/debug`).
This will take some time as it builds `QEMU`.

Second, you have to prepare the nyx directories including the libxml2 target by running `setup_libxml2.sh`. 
You will have some missing dependencies, so install them if needed.
This step could also take some time as it build `AFL++` and `libxml2`, so grab a coffee meanwhile.
This will create nyx directories inside `/tmp/nyx_libxml2/`

## Running

After all is ready, you can just run `target/debug/baby_nyx`

## Code

Essentially the only two unique parts to the Nyx fuzzers are 

```rust
    let settings = NyxSettings::builder().cpu_id(0).parent_cpu_id(None).build();
    let helper = NyxHelper::new("/tmp/nyx_libxml2/", settings).unwrap();
    let observer =
        unsafe { StdMapObserver::from_mut_ptr("trace", helper.bitmap_buffer, helper.bitmap_size) };
```

Here we setup a `NyxSettings` object to describe what options we want to use for spawning a nyx process.
`NyxHelper` can be constructed by taking the path to the nyx directories that we prepared earlier.
The coverage map can be obtained from the `helper.bitmap_size`

After this setup,
```rust
    let executor = NyxExecutor::builder().build(helper, tuple_list!(observer));
```
This code will create an instance of `NyxExecutor`.

The rest of the fuzzer code should be something that we've already covered on non-nyx examples.