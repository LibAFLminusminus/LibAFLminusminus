# Intel Processor Trace (PT) low level code

This module is a wrapper around the `IntelPT` kernel driver, exposing functionalities specifically crafted for `LibAFL`.

At the moment only `Linux` hosts are supported.

You can run `sudo -E cargo test intel_pt_check_availability -- --show-output` to check if your host has all the features
used by this crate.

## The `LibAFLmm` Project

This crate is part of the [LibAFLmm project](https://github.com/LibAFLminusminus/LibAFLminusminus).

The [README](../../README.md) contains the list of maintainers and licensing information.
