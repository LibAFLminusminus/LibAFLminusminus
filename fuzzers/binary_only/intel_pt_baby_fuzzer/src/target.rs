use libaflmm::{Result, executors::ExitKind, inputs::BytesInput};
use std::hint::black_box;

// Coverage map
pub const MAP_SIZE: usize = 4096;
pub static mut MAP: [u8; MAP_SIZE] = [0; MAP_SIZE];
pub static mut MAP_PTR: *mut u8 = &raw mut MAP as _;

// The closure that we want to fuzz
pub fn target<S>(_state: &mut S, input: &BytesInput) -> Result<ExitKind> {
    let target = input.as_ref();
    let buf = target.as_slice();

    if !buf.is_empty() && buf[0] == b'a' {
        let _do_something = black_box(0);
        if buf.len() > 1 && buf[1] == b'b' {
            let _do_something = black_box(0);
            if buf.len() > 2 && buf[2] == b'c' {
                panic!("Artificial bug triggered =)");
            }
        }
    }

    Ok(ExitKind::Ok)
}
