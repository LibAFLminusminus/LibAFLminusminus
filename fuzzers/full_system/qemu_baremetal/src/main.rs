//! A binary-only systemmode fuzzer using qemu for binary-only coverage

use libaflmm::Result;

#[cfg(feature = "low_level")]
mod fuzzer_low_level;

#[cfg(feature = "breakpoint")]
mod fuzzer_breakpoint;

#[cfg(feature = "custom_insn")]
mod fuzzer_custom_insn;

pub fn main() -> Result<()> {
    #[cfg(feature = "low_level")]
    fuzzer_low_level::fuzz()?;

    #[cfg(feature = "breakpoint")]
    fuzzer_breakpoint::fuzz()?;

    #[cfg(feature = "custom_insn")]
    fuzzer_custom_insn::fuzz()?;

    Ok(())
}
