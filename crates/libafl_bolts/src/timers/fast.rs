//! Cross-platform fast timer

use core::{sync::atomic::AtomicU64, time::Duration};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use libafl_core::Result;
use libc::{SIGALRM, pthread_kill, pthread_self};

use crate::timers::Timer;

/// Default timer resolution
pub const DEFAULT_RESOLUTION: TimerResolution = TimerResolution::Factor(10);

/// The timer resolution represents its check granularity.
/// The timer will only notice it has timed out with the precision
/// specificed by the resolution.
#[derive(Debug, Clone)]
pub enum TimerResolution {
    /// A fixed resolution
    Fixed(Duration),
    /// A dynamic resolution.
    /// It will be the timeout value divided by this factor.
    Factor(u128),
}

struct SharedState {
    is_active: AtomicBool,
    arm_counter: AtomicU64,
    should_stop: AtomicBool,
}

/// A fast userland timer backed by a polling thread.
///
/// The thread wakes every `resolution` and checks whether the deadline has passed.
/// On expiry it raises `SIGALRM` (Unix) in the calling process.
#[derive(Debug)]
pub struct FastTimer {
    requested_resolution: TimerResolution,
    timeout: Duration,
    resolution: Duration,
    shared: Option<Arc<SharedState>>,
    timer: Option<JoinHandle<Result<()>>>,
}

impl core::fmt::Debug for SharedState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SharedState")
            .field("is_active", &self.is_active)
            .field("should_stop", &self.should_stop)
            .finish_non_exhaustive()
    }
}

impl Clone for FastTimer {
    fn clone(&self) -> Self {
        Self {
            requested_resolution: self.requested_resolution.clone(),
            timeout: self.timeout,
            resolution: self.resolution,
            shared: None,
            timer: None,
        }
    }
}

impl TimerResolution {
    /// Get a fixed timer resolution
    pub fn fixed(resolution: Duration) -> Self {
        Self::Fixed(resolution)
    }

    /// Get a dynamic timer resolution
    pub fn factor(factor: usize) -> Self {
        Self::Factor(factor as u128)
    }

    /// Get the real duration from the resolution.
    pub fn to_duration(&self, timeout: &Duration) -> Duration {
        match self {
            TimerResolution::Fixed(fixed) => *fixed,
            TimerResolution::Factor(factor) => {
                Duration::from_nanos((timeout.as_nanos() / factor) as u64)
            }
        }
    }
}

impl FastTimer {
    /// Create a fast timer.
    pub fn new() -> Self {
        Self {
            resolution: Duration::default(),
            timeout: Duration::default(),
            requested_resolution: DEFAULT_RESOLUTION,
            shared: None,
            timer: None,
        }
    }
}

impl Timer for FastTimer {
    fn create_timer(&mut self, timeout: Duration) -> Result<()> {
        if self.timer.is_some() {
            self.delete_timer()?;
        }

        self.timeout = timeout;
        self.resolution = self.requested_resolution.to_duration(&timeout);

        let shared_clock = Arc::new(SharedState {
            is_active: AtomicBool::new(false),
            should_stop: AtomicBool::new(false),
            arm_counter: AtomicU64::new(0),
        });

        let shared: Arc<_> = shared_clock.clone();
        let resolution = self.resolution;
        let timeout = self.timeout;

        let thread_id = unsafe { pthread_self() };

        let handle = thread::spawn(move || {
            let mut start = Instant::now();
            let mut current_counter = 0;

            let mut is_active = false;

            loop {
                thread::sleep(resolution);

                // first, check if we should stop because the timer got killed.
                if shared.should_stop.load(Ordering::Relaxed) {
                    break;
                }

                // if not actively clocking and the main thread is getting active,
                // save the current counter and start clocking.
                if !is_active && shared.is_active.load(Ordering::Relaxed) {
                    is_active = true;
                    current_counter = shared.arm_counter.load(Ordering::Relaxed);
                    start = Instant::now();
                }

                if is_active {
                    // the clock thread is active

                    if !shared.is_active.load(Ordering::Relaxed) {
                        // the main thread went inactive while we were sleeping.

                        is_active = false;
                    } else if current_counter != shared.arm_counter.load(Ordering::Relaxed) {
                        // the tracked run finished while we were sleeping

                        debug_assert!(current_counter < shared.arm_counter.load(Ordering::Relaxed));
                        is_active = false;
                    } else {
                        // active and in sync with the main thread, now check timeout

                        if Instant::now().duration_since(start) > timeout {
                            // issue the timeout
                            unsafe {
                                // kill the main thread with the correct signal
                                pthread_kill(thread_id, SIGALRM);
                            }

                            shared.is_active.store(false, Ordering::Relaxed);
                            is_active = false;
                        }
                    }
                }
            }

            Ok(())
        });

        self.shared = Some(shared_clock);
        self.timer = Some(handle);

        Ok(())
    }

    unsafe fn arm_timer(&mut self) -> Result<()> {
        if let Some(shared) = &self.shared {
            shared.arm_counter.fetch_add(1, Ordering::Relaxed);
            shared.is_active.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    unsafe fn disarm_timer(&mut self) -> Result<()> {
        if let Some(shared) = &self.shared {
            shared.is_active.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    fn delete_timer(&mut self) -> Result<()> {
        if let Some(shared) = &self.shared {
            shared.should_stop.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.timer.take() {
            handle.join().unwrap()?;
        }
        self.shared = None;
        Ok(())
    }
}
