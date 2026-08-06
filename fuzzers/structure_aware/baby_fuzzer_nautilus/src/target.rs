use libaflmm::{Result, executors::ExitKind, inputs::NautilusInput, states::State};
use std::ptr::write;

/// Coverage map with explicit assignments due to the lack of instrumentation
pub const SIGNALS_LEN: usize = 16;
pub static mut SIGNALS: [u8; SIGNALS_LEN] = [0; SIGNALS_LEN];
pub static mut SIGNALS_PTR: *mut u8 = &raw mut SIGNALS as _;

/// Assign a signal to the signals map
fn signals_set(idx: usize) {
    unsafe { write(SIGNALS_PTR.add(idx), 1) };
}

// The closure that we want to fuzz
pub fn target<S>(state: &mut S, input: &NautilusInput) -> Result<ExitKind>
where
    S: State<Input = NautilusInput>,
{
    // the state knows the grammar, it unparses the input for us
    let bytes = state.input_to_bytes(input);

    println!(">>> {}", String::from_utf8_lossy(&bytes));

    // there is nothing to instrument here, so the size of the generated program is
    // used as a stand-in coverage: the fuzzer gets something to maximize.
    signals_set(bytes.len() % SIGNALS_LEN);

    Ok(ExitKind::Ok)
}
