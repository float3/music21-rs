//! music21's `common.objects`, as far as it is not about Python objects.

use std::time::{Duration, Instant};

/// music21's `Timer`: how long since it was started, or, once stopped, how
/// long it ran.
///
/// ```
/// use music21::common::objects::Timer;
///
/// let mut timer = Timer::new();
/// timer.stop();
/// assert_eq!(timer.elapsed(), timer.elapsed());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Timer {
    started: Instant,
    ran: Option<Duration>,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    /// A timer started now.
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            ran: None,
        }
    }

    /// Starts again from now, running.
    pub fn start(&mut self) {
        *self = Self::new();
    }

    /// Stops, keeping how long it ran; stopping again keeps the later time.
    pub fn stop(&mut self) {
        self.ran = Some(self.started.elapsed());
    }

    /// The same as [`Timer::start`], which is what music21's `clear` does.
    pub fn clear(&mut self) {
        self.start();
    }

    /// Seconds since it started, or the seconds it ran if stopped.
    pub fn elapsed(&self) -> f64 {
        self.ran
            .unwrap_or_else(|| self.started.elapsed())
            .as_secs_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stopped timer stands still, and starting it again runs it from
    /// nothing.
    #[test]
    fn a_stopped_timer_stands_still_until_started() {
        let mut timer = Timer::new();
        std::thread::sleep(Duration::from_millis(2));
        timer.stop();
        let ran = timer.elapsed();
        assert!(ran > 0.0);
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(timer.elapsed(), ran);
        timer.start();
        assert!(timer.elapsed() < ran + 0.002);
        assert!(timer.ran.is_none());
    }
}
