//! The struct `TimerStruct` will absorb all the difference in timeout implementation in various system.
use core::time::Duration;
use nix::{
    sys::{
        signal::{SigEvent, SigevNotify, Signal},
        time::{TimeSpec, TimeValLike},
        timer::{self, Expiration, TimerSetTimeFlags},
    },
    time::ClockId,
};

use crate::timers::Timer;

/// The strcut about all the internals of the timer.
/// This struct absorb all platform specific differences about timer.
#[derive(Debug)]
pub struct StdTimer {
    timer: Option<(timer::Timer, Expiration)>,
    disable: Expiration,
}

impl Clone for StdTimer {
    fn clone(&self) -> Self {
        Self {
            timer: None,
            disable: self.disable.clone(),
        }
    }
}

impl StdTimer {
    /// Create a `TimerStruct` with the specified timeout.
    #[must_use]
    pub fn new() -> Self {
        let disable = Expiration::OneShot(TimeSpec::zero());

        Self {
            timer: None,
            disable,
        }
    }
}

impl Timer for StdTimer {
    fn create_timer(&mut self, timeout: Duration) -> libafl_core::Result<()> {
        if self.timer.is_some() {
            self.delete_timer()?;
        }

        let expiration = Expiration::OneShot(TimeSpec::from_duration(timeout));

        let sigevent = SigEvent::new(SigevNotify::SigevSignal {
            signal: Signal::SIGALRM,
            si_value: 0,
        });
        self.timer = Some((
            timer::Timer::new(ClockId::CLOCK_MONOTONIC, sigevent)?,
            expiration,
        ));

        Ok(())
    }

    unsafe fn arm_timer(&mut self) -> libafl_core::Result<()> {
        if let Some((timer, expiration)) = &mut self.timer {
            let flags = TimerSetTimeFlags::empty();

            timer.set(expiration.clone(), flags)?;
        }

        Ok(())
    }

    unsafe fn disarm_timer(&mut self) -> libafl_core::Result<()> {
        if let Some((timer, _)) = &mut self.timer {
            let flags = TimerSetTimeFlags::empty();

            timer.set(self.disable.clone(), flags)?;
        }

        Ok(())
    }

    fn delete_timer(&mut self) -> libafl_core::Result<()> {
        self.timer.take();

        Ok(())
    }
}
