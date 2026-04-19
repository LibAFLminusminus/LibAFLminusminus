use std::ptr::write;

use libafl::{Error, executors::ExitKind, inputs::BytesInput};

/// Coverage map with explicit assignments due to the lack of instrumentation
pub const SIGNALS_LEN: usize = 16;
pub static mut SIGNALS: [u8; SIGNALS_LEN] = [0; SIGNALS_LEN];
pub static mut SIGNALS_PTR: *mut u8 = &raw mut SIGNALS as _;

/// Assign a signal to the signals map
fn signals_set(idx: usize) {
    unsafe { write(SIGNALS_PTR.add(idx), 1) };
}

// The closure that we want to fuzz
pub fn target<S>(_state: &mut S, input: &BytesInput) -> Result<ExitKind, Error> {
    let target = input.as_ref();
    let buf = target.as_slice();
    signals_set(0);
    if !buf.is_empty() && buf[0] == b'a' {
        signals_set(1);
        if buf.len() > 1 && buf[1] == b'b' {
            signals_set(2);
            if buf.len() > 2 && buf[2] == b'c' {
                #[cfg(unix)]
                panic!("Artificial bug triggered =)");

                // panic!() raises a STATUS_STACK_BUFFER_OVERRUN exception which cannot be caught by the exception handler.
                // Here we make it raise STATUS_ACCESS_VIOLATION instead.
                // Extending the windows exception handler is a TODO. Maybe we can refer to what winafl code does.
                // https://github.com/googleprojectzero/winafl/blob/ea5f6b85572980bb2cf636910f622f36906940aa/winafl.c#L728
                #[cfg(windows)]
                unsafe {
                    // Replace zero-ptr with the below function, suggested by Clippy
                    write_volatile(std::ptr::null_mut::<u32>(), 0);
                }
            }
        }
    }

    Ok(ExitKind::Ok)
}
