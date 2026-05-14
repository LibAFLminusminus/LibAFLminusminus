#!/bin/sh
./target/release/libafl_cc -c -o target.o ./target.c
./target/release/libafl_cc -o fuzzer ./target.o 
