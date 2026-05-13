//! Time functions that can be used on `no_std`
#![doc = include_str!("../README.md")]
/*! */
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![cfg_attr(not(test), warn(
    missing_debug_implementations,
    missing_docs,
    //trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    //unused_results
))]
#![cfg_attr(test, deny(
    missing_debug_implementations,
    missing_docs,
    //trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_must_use,
    //unused_results
))]
#![cfg_attr(
    test,
    deny(
        bad_style,
        dead_code,
        improper_ctypes,
        non_shorthand_field_patterns,
        no_mangle_generic_items,
        overflowing_literals,
        path_statements,
        patterns_in_fns_without_body,
        unconditional_recursion,
        unused,
        unused_allocation,
        unused_comparisons,
        unused_parens,
        while_true
    )
)]

use alloc::string::String;
use core::time;
use std::time::{SystemTime, UNIX_EPOCH};

/// Format a `Duration` into a HMS string
#[must_use]
pub fn format_duration(duration: &time::Duration) -> String {
    const MINS_PER_HOUR: u64 = 60;
    const HOURS_PER_DAY: u64 = 24;

    const SECS_PER_MINUTE: u64 = 60;
    const SECS_PER_HOUR: u64 = SECS_PER_MINUTE * MINS_PER_HOUR;
    const SECS_PER_DAY: u64 = SECS_PER_HOUR * HOURS_PER_DAY;

    let total_secs = duration.as_secs();
    let secs = total_secs % SECS_PER_MINUTE;

    if total_secs < SECS_PER_MINUTE {
        format!("{secs}s")
    } else {
        let mins = (total_secs / SECS_PER_MINUTE) % MINS_PER_HOUR;
        if total_secs < SECS_PER_HOUR {
            format!("{mins}m-{secs}s")
        } else {
            let hours = (total_secs / SECS_PER_HOUR) % HOURS_PER_DAY;
            if total_secs < SECS_PER_DAY {
                format!("{hours}h-{mins}m-{secs}s")
            } else {
                let days = total_secs / SECS_PER_DAY;
                format!("{days}days {hours}h-{mins}m-{secs}s")
            }
        }
    }
}

#[cfg(all(any(doctest, test), not(feature = "std")))]
/// Provide custom time in `no_std` tests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn external_current_millis() -> u64 {
    // TODO: use "real" time here
    1000
}

/// Current time
#[must_use]
#[inline]
pub fn current_time() -> time::Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}

/// Gets current nanoseconds since [`UNIX_EPOCH`]
#[must_use]
#[inline]
pub fn current_nanos() -> u64 {
    current_time().as_nanos() as u64
}

/// Gets current milliseconds since [`UNIX_EPOCH`]
#[must_use]
#[inline]
pub fn current_milliseconds() -> u64 {
    current_time().as_millis() as u64
}
