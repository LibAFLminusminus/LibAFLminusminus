# Snapshot Fuzzing in Nyx

Nyx is an execution fuzzing engine for snapshot fuzzing.
In `libaflmm_nyx`, we only supports fuzzing with compile-time instrumentation using [afl++](https://github.com/AFLplusplus/AFLplusplus).
Therefore you need to have your source code and instrument it with AFLpp's compiler. 

## Depenedencies
`libaflmm_nyx` has quite a few dependencies since it needs to build QEMU first.
Try to run `build_nyx_support.sh` first to install all the missing dependencies before making a nyx fuzzer

## Preparing the Nyx working directory

The first step is to pack the target into Nyx's kernel.
We have a template shell script in our [example](https://github.com/LibAFLminusminus/LibAFLminusminus/blob/main/fuzzers/baby/baby_nyx/setup_libxml2.sh):

The important part is just to run the `nyx_packer.py` script with the correct arguments.

```bash
python3 "./packer/packer/nyx_packer.py" \
    ./libxml2/xmllint \   # your target binary
    /tmp/nyx_libxml2 \    # the nyx work directory
    afl \                 # instrumentation type
    instrumentation \
    -args "/tmp/input" \  # the args of the program. it means that we will run `xmllint /tmp/input` in each run.
    -file "/tmp/input" \  # the input will be generated in `/tmp/input`. If no `-file` is given, then input will be passed through stdin
    --fast_reload_mode \
    --purge || exit
```

Then, you can generate the config file:

```bash
python3 ./packer/packer/nyx_config_gen.py /tmp/nyx_libxml2/ Kernel || exit
```

## Example

We will walk you through a concrete example in another [section](../examples/nyx.md)