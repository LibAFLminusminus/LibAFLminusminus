#![cfg(target_arch = "wasm32")]

//! Test suite for the Web and headless browsers.

use baby_fuzzer_wasm::fuzz;
use wasm_bindgen_test::*;

extern crate wasm_bindgen_test;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_fuzz() {
    fuzz();
}
