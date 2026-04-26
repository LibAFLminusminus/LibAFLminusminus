//! Cross-platform fast timer

use core::time::Duration;

/// A fast timer
#[derive(Debug)]
#[expect(dead_code)]
pub struct FastTimer {
    resolution: Duration,
    timeout: Duration,
}

impl FastTimer {
    /// Create a fast timer.
    pub fn new() -> Self {
        Self {
            resolution: Duration::default(),
            timeout: Duration::default(),
        }
    }
}
