// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant};

/// An owned monotonic clock measuring active elapsed time.
///
/// A `Clock` captures a starting [`Instant`] and accumulates the active intervals during which it
/// runs. The application owns its loop: event polling, drawing, presenting, and any other clock
/// never change this clock's state, and reading a clock never schedules a callback or advances a
/// simulation. Each `Clock` is independent of every other instance.
///
/// The time source is [`std::time::Instant`], not `SystemTime`, GLFW's global timer, or GPU
/// timestamps. Source resolution, accuracy, monotonicity, and system-suspension treatment inherit
/// [the platform limits of `Instant`](https://doc.rust-lang.org/std/time/struct.Instant.html);
/// nanosecond duration storage does not guarantee nanosecond source accuracy. Time between client
/// samples includes whatever the application performed between them; this is an elapsed-time
/// measurement, not CPU/GPU profiling isolation.
///
/// # State model
///
/// A clock is either running or paused. While running, [`elapsed`](Clock::elapsed) reports the
/// accumulated active time plus the current running interval. While paused, it reports exactly the
/// frozen accumulated value without consulting the OS time source. Interval arithmetic saturates:
/// a backwards source interval contributes zero and total accumulation saturates at
/// [`Duration::MAX`]; neither wraps nor panics. These are representational limits, not a gameplay
/// delta clamp.
///
/// # Costs and threading
///
/// Construction, reads, and transitions perform no heap allocation, spawn no work, acquire no VMNL
/// global locks, initialize no GLFW/Vulkan backend, and never block waiting for a frame. `Clock`
/// is `Send + Sync` through the standard value types it stores; mutation follows ordinary Rust
/// borrowing (`&mut self`) or external synchronization. `Clock` is deliberately not `Copy` or
/// `Clone`: duplicating a clock would create ambiguous ownership of the same measurement.
///
/// # Example
///
/// Independent measurements with pause/resume, without any window:
///
/// ```
/// use vmnl_graphics::time::Clock;
///
/// let mut frame_clock = Clock::new();
/// let mut measurement_clock = Clock::new();
///
/// measurement_clock.pause();
/// let frozen = measurement_clock.elapsed();
/// let _frame_dt = frame_clock.restart();
/// assert_eq!(measurement_clock.elapsed(), frozen);
/// assert!(measurement_clock.is_paused());
///
/// measurement_clock.pause(); // Still frozen.
/// measurement_clock.resume(); // Continue from frozen; exclude the paused interval.
/// assert!(!measurement_clock.is_paused());
///
/// measurement_clock.pause();
/// let before_restart = measurement_clock.elapsed();
/// let returned = measurement_clock.restart();
/// assert_eq!(returned, before_restart);
/// assert!(!measurement_clock.is_paused());
/// ```
pub struct Clock {
    /// Completed active intervals.
    accumulated: Duration,
    /// Origin of the current running interval; `None` while paused.
    running_since: Option<Instant>,
}

impl Clock {
    /// Creates a running clock with zero accumulated time.
    ///
    /// The starting instant is captured now, so a later [`elapsed`](Clock::elapsed) may already be
    /// nonzero. See [`Clock::default`] for the identical `Default` implementation.
    #[must_use]
    pub fn new() -> Self {
        Self::from_origin(Instant::now())
    }

    /// Returns the active elapsed time without changing the clock's origin or state.
    ///
    /// While running, this is the accumulated active intervals plus the current running interval;
    /// the call does not consume the elapsed time. While paused, this is exactly the frozen
    /// accumulated value, without consulting the OS time source. The result is a
    /// [`Duration`] saturating at [`Duration::MAX`].
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        match self.running_since {
            Some(_) => self.elapsed_at(Instant::now()),
            None => self.accumulated,
        }
    }

    /// Returns the active elapsed time through the restart boundary, resets the accumulation to
    /// zero, and leaves the clock running.
    ///
    /// Works from either state: while paused, the frozen accumulated value is returned. One
    /// captured instant both finishes the old interval and starts the new one, so no time is lost
    /// or double-counted across the restart. This is the usual way to sample a per-frame delta.
    ///
    /// # Example
    ///
    /// ```
    /// use vmnl_graphics::time::{Clock, Duration};
    ///
    /// fn update_movement(_dt: Duration) { /* client-owned logic */ }
    /// fn update_animation(_dt: Duration) { /* client-owned logic */ }
    ///
    /// let mut frame_clock = Clock::new();
    ///
    /// // At the beginning of each application frame:
    /// let dt = frame_clock.restart();
    /// update_movement(dt);
    /// update_animation(dt);
    /// ```
    #[must_use]
    pub fn restart(&mut self) -> Duration {
        self.restart_at(Instant::now())
    }

    /// Accumulates active time through one captured pause instant and enters the paused state.
    ///
    /// Idempotent: calling `pause` on an already paused clock preserves the frozen duration and
    /// state without reading the OS time source.
    pub fn pause(&mut self) {
        if self.running_since.is_some() {
            self.pause_at(Instant::now());
        }
    }

    /// Preserves the accumulated duration, sets a new running origin, and excludes the paused
    /// interval from future measurements.
    ///
    /// Idempotent: calling `resume` on an already running clock does not reset the origin or
    /// discard time, and does not read the OS time source.
    pub fn resume(&mut self) {
        if self.running_since.is_none() {
            self.resume_at(Instant::now());
        }
    }

    /// Returns whether the clock is paused, without affecting the measurement.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.running_since.is_none()
    }

    fn from_origin(origin: Instant) -> Self {
        Self {
            accumulated: Duration::ZERO,
            running_since: Some(origin),
        }
    }

    fn elapsed_at(&self, now: Instant) -> Duration {
        match self.running_since {
            Some(origin) => self
                .accumulated
                .saturating_add(now.saturating_duration_since(origin)),
            None => self.accumulated,
        }
    }

    fn pause_at(&mut self, now: Instant) {
        if let Some(origin) = self.running_since {
            self.accumulated = self
                .accumulated
                .saturating_add(now.saturating_duration_since(origin));
            self.running_since = None;
        }
    }

    fn resume_at(&mut self, now: Instant) {
        if self.running_since.is_none() {
            self.running_since = Some(now);
        }
    }

    fn restart_at(&mut self, now: Instant) -> Duration {
        let elapsed = self.elapsed_at(now);
        self.accumulated = Duration::ZERO;
        self.running_since = Some(now);
        elapsed
    }
}

impl Default for Clock {
    /// Identical to [`Clock::new`]: a running clock capturing the current instant with zero
    /// accumulated time.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn at(base: Instant, ms: u32) -> Instant {
        base + MS * ms
    }

    #[test]
    fn running_reads_at_supplied_instants_do_not_consume_elapsed_time() {
        let t0 = Instant::now();
        let clock = Clock::from_origin(t0);

        assert_eq!(clock.elapsed_at(at(t0, 10)), 10 * MS);
        assert_eq!(clock.elapsed_at(at(t0, 10)), 10 * MS);
        assert_eq!(clock.elapsed_at(at(t0, 25)), 25 * MS);
        assert!(!clock.is_paused());
    }

    #[test]
    fn pause_freezes_the_accumulated_value_exactly() {
        let t0 = Instant::now();
        let mut clock = Clock::from_origin(t0);

        clock.pause_at(at(t0, 10));

        assert!(clock.is_paused());
        assert_eq!(clock.elapsed_at(at(t0, 30)), 10 * MS);
        assert_eq!(clock.elapsed_at(at(t0, 30)), 10 * MS);
    }

    #[test]
    fn resume_excludes_the_paused_interval_and_restart_reuses_the_boundary() {
        let t0 = Instant::now();
        let mut clock = Clock::from_origin(t0);

        clock.pause_at(at(t0, 10));
        clock.resume_at(at(t0, 30));

        assert_eq!(clock.elapsed_at(at(t0, 35)), 15 * MS);
        assert_eq!(clock.restart_at(at(t0, 35)), 15 * MS);
        assert_eq!(clock.elapsed_at(at(t0, 40)), 5 * MS);
        assert!(!clock.is_paused());
    }

    #[test]
    fn repeated_pause_and_resume_are_idempotent() {
        let t0 = Instant::now();
        let mut clock = Clock::from_origin(t0);

        clock.pause_at(at(t0, 10));
        clock.pause_at(at(t0, 20));
        assert_eq!(clock.elapsed_at(at(t0, 30)), 10 * MS);

        clock.resume_at(at(t0, 30));
        clock.resume_at(at(t0, 31));
        assert_eq!(clock.elapsed_at(at(t0, 35)), 15 * MS);
        assert_eq!(clock.restart_at(at(t0, 35)), 15 * MS);
    }

    #[test]
    fn restart_while_paused_returns_the_frozen_value_and_runs_from_zero() {
        let t0 = Instant::now();
        let mut clock = Clock::from_origin(t0);

        clock.pause_at(at(t0, 10));
        assert_eq!(clock.restart_at(at(t0, 40)), 10 * MS);

        assert!(!clock.is_paused());
        assert_eq!(clock.elapsed_at(at(t0, 40)), Duration::ZERO);
        assert_eq!(clock.elapsed_at(at(t0, 45)), 5 * MS);
    }

    #[test]
    fn equal_and_backwards_instants_contribute_zero_without_underflow() {
        let t0 = Instant::now();
        let clock = Clock::from_origin(t0);

        assert_eq!(clock.elapsed_at(t0), Duration::ZERO);

        let later_origin = Clock::from_origin(at(t0, 10));
        assert_eq!(later_origin.elapsed_at(t0), Duration::ZERO);
        assert_eq!(later_origin.elapsed_at(at(t0, 10)), Duration::ZERO);
    }

    #[test]
    fn accumulation_saturates_at_duration_max_without_wrap_or_panic() {
        let t0 = Instant::now();
        let mut clock = Clock {
            accumulated: Duration::MAX.saturating_sub(5 * MS),
            running_since: Some(t0),
        };

        assert_eq!(clock.elapsed_at(at(t0, 10)), Duration::MAX);
        assert_eq!(clock.restart_at(at(t0, 10)), Duration::MAX);
        assert_eq!(clock.elapsed_at(at(t0, 20)), 10 * MS);

        let mut paused = Clock {
            accumulated: Duration::MAX.saturating_sub(5 * MS),
            running_since: Some(t0),
        };
        paused.pause_at(at(t0, 10));
        assert_eq!(paused.elapsed_at(at(t0, 30)), Duration::MAX);
    }

    #[test]
    fn restarting_or_pausing_one_clock_leaves_another_untouched() {
        let t0 = Instant::now();
        let mut first = Clock::from_origin(t0);
        let mut second = Clock::from_origin(t0);

        assert_eq!(first.restart_at(at(t0, 10)), 10 * MS);
        second.pause_at(at(t0, 20));

        assert_eq!(first.elapsed_at(at(t0, 15)), 5 * MS);
        assert_eq!(second.elapsed_at(at(t0, 50)), 20 * MS);
        assert!(!first.is_paused());
        assert!(second.is_paused());
    }
}
