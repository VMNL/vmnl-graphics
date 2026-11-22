// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use crate::{VMNLError, VMNLErrorKind, VMNLResult};

/// Whether a [`Timer`] fires once or periodically.
///
/// The mode is fixed at construction and returned unchanged by [`Timer::mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    /// A one-shot timer that completes once its duration is reached and stays finished until
    /// [`Timer::restart`].
    Once,
    /// A repeating timer that counts every crossed period in [`Timer::advance`] and never
    /// finishes on its own.
    Repeating,
}

/// A duration-driven timer advanced explicitly by the caller.
///
/// A `Timer` accumulates a [`Duration`] supplied by its caller through [`advance`](Timer::advance).
/// It never reads a wall or monotonic clock, owns no application loop, triggers no callback, and
/// spawns no thread: the same delta can advance several timers, and clients choose real-time,
/// paused, scaled, fixed-step, or synthetic deltas. A timer's pause state is independent of any
/// source clock (such as [`Clock`](crate::time::Clock)) and of every other timer.
///
/// # State model
///
/// The private state is the configured `duration` and `mode`, the current cycle `elapsed`, and a
/// `paused` flag, with the invariant `elapsed <= duration` (and `elapsed < duration` for a running
/// repeating timer). A finished one-shot is exactly `elapsed == duration`; there is no separate
/// completion flag or stored event queue. [`advance`](Timer::advance) returns the number of
/// duration boundaries crossed by that call as a `u128`, so a one-nanosecond period advanced by
/// [`Duration::MAX`] is counted without truncation.
///
/// # Costs and threading
///
/// Construction and updates perform bounded integer arithmetic per call, independent of the
/// number of crossed periods; they require no window, Vulkan, GLFW, OS clock, worker, callback,
/// sleep, lock, GPU submission, or heap allocation on the successful path (the rejected
/// zero-duration construction may allocate the structured error's message). `Timer` is
/// `Send + Sync` through the standard value types it stores; mutation follows ordinary Rust
/// borrowing. `Timer` is deliberately not `Copy` or `Clone`.
///
/// # Example
///
/// One-shot cooldown driven by a caller-supplied delta:
///
/// ```
/// use vmnl_graphics::time::{Duration, Timer, TimerMode};
/// use vmnl_graphics::VMNLResult;
///
/// fn main() -> VMNLResult<()> {
///     let mut cooldown = Timer::new(Duration::from_millis(250), TimerMode::Once)?;
///     assert_eq!(cooldown.advance(Duration::from_millis(100)), 0);
///     assert_eq!(cooldown.remaining(), Duration::from_millis(150));
///     assert!(!cooldown.is_finished());
///
///     assert_eq!(cooldown.advance(Duration::from_millis(200)), 1);
///     assert!(cooldown.is_finished());
///     assert_eq!(cooldown.elapsed(), Duration::from_millis(250));
///     assert_eq!(cooldown.remaining(), Duration::ZERO);
///     assert_eq!(cooldown.advance(Duration::from_secs(1)), 0);
///
///     cooldown.restart();
///     assert!(!cooldown.is_finished());
///     assert_eq!(cooldown.elapsed(), Duration::ZERO);
///     Ok(())
/// }
/// ```
pub struct Timer {
    /// Configured period; strictly positive by construction.
    duration: Duration,
    /// Configured firing mode; immutable after construction.
    mode: TimerMode,
    /// Progress within the current cycle; `<= duration` by invariant.
    elapsed: Duration,
    /// Whether supplied deltas are currently ignored.
    paused: bool,
}

impl Timer {
    /// Creates an unpaused timer with zero elapsed time and full remaining duration.
    ///
    /// `duration` must be strictly positive for both modes, including [`Duration::MAX`]; zero is
    /// rejected with [`VMNLErrorKind::InvalidState`] and the diagnostic
    /// `timer duration must be strictly positive`. There is intentionally no `Default`: the period
    /// and mode are always explicit.
    ///
    /// # Errors
    ///
    /// Returns [`VMNLErrorKind::InvalidState`] with the message
    /// `timer duration must be strictly positive` when `duration` is [`Duration::ZERO`].
    ///
    /// # Example
    ///
    /// ```
    /// use vmnl_graphics::time::{Duration, Timer, TimerMode};
    /// use vmnl_graphics::VMNLErrorKind;
    ///
    /// let result = Timer::new(Duration::ZERO, TimerMode::Repeating);
    /// assert!(matches!(
    ///     result,
    ///     Err(error) if matches!(error.kind(),
    ///         VMNLErrorKind::InvalidState(message)
    ///         if message == "timer duration must be strictly positive")
    /// ));
    /// ```
    pub fn new(duration: Duration, mode: TimerMode) -> VMNLResult<Self> {
        if duration == Duration::ZERO {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "timer duration must be strictly positive".to_string(),
            )));
        }

        Ok(Self {
            duration,
            mode,
            elapsed: Duration::ZERO,
            paused: false,
        })
    }

    /// Advances the timer by the supplied `delta` and returns the number of duration boundaries
    /// crossed by this call.
    ///
    /// While paused, the delta is ignored and `0` is returned; [`Duration::ZERO`] also returns `0`.
    /// A `Once` timer returns `1` when the boundary is reached or crossed (clamping `elapsed` to
    /// the duration and making completion sticky) and `0` before that or after completion. A
    /// `Repeating` timer counts every crossed period exactly and preserves the fractional
    /// remainder for the next cycle. The count is not a queue: save or share the returned value if
    /// several client systems need the same observation.
    ///
    /// # Example
    ///
    /// ```
    /// use vmnl_graphics::time::{Duration, Timer, TimerMode};
    /// use vmnl_graphics::VMNLResult;
    ///
    /// fn main() -> VMNLResult<()> {
    ///     let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;
    ///     let crossings: u128 = timer.advance(Duration::from_millis(350));
    ///     assert_eq!(crossings, 3);
    ///     assert_eq!(timer.elapsed(), Duration::from_millis(50));
    ///     assert_eq!(timer.remaining(), Duration::from_millis(50));
    ///     assert!(!timer.is_finished());
    ///
    ///     assert_eq!(timer.advance(Duration::from_millis(50)), 1);
    ///     assert_eq!(timer.elapsed(), Duration::ZERO);
    ///     assert_eq!(timer.remaining(), Duration::from_millis(100));
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn advance(&mut self, delta: Duration) -> u128 {
        if self.paused {
            return 0;
        }

        match self.mode {
            TimerMode::Once => self.advance_once(delta),
            TimerMode::Repeating => self.advance_repeating(delta),
        }
    }

    /// Returns the configured period without changing state.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Returns the configured mode without changing state.
    #[must_use]
    pub fn mode(&self) -> TimerMode {
        self.mode
    }

    /// Returns the progress within the current cycle without consuming anything.
    ///
    /// For `Once` this is in `[0, duration]`; for `Repeating` in `[0, duration)`. It is not
    /// lifetime elapsed time.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Returns `duration - elapsed`: the time left in the current cycle, without consuming
    /// anything.
    ///
    /// Zero for a completed `Once` timer; in `(0, duration]` for `Repeating`, where an exact
    /// boundary starts a new full period.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        // The state invariant is elapsed <= duration, so the subtraction always succeeds.
        self.duration
            .checked_sub(self.elapsed)
            .unwrap_or(Duration::ZERO)
    }

    /// Returns whether the timer is finished: only a completed `Once` timer is ever finished.
    ///
    /// `Repeating` timers are never finished; their periodic boundary events come from
    /// [`advance`](Timer::advance)'s return value.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.mode == TimerMode::Once && self.elapsed == self.duration
    }

    /// Resets elapsed time and completion and enters the unpaused state, from any prior state.
    ///
    /// The configured duration and mode are unchanged. Unlike [`Clock::restart`](crate::time::Clock::restart),
    /// this returns `()` because a timer's boundary events are reported by [`advance`](Timer::advance).
    pub fn restart(&mut self) {
        self.elapsed = Duration::ZERO;
        self.paused = false;
    }

    /// Pauses the timer, preserving progress and completion.
    ///
    /// Idempotent: pausing an already paused timer changes nothing. While paused,
    /// [`advance`](Timer::advance) ignores the supplied delta instead of queueing it.
    ///
    /// # Example
    ///
    /// ```
    /// use vmnl_graphics::time::{Duration, Timer, TimerMode};
    /// use vmnl_graphics::VMNLResult;
    ///
    /// fn main() -> VMNLResult<()> {
    ///     let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;
    ///     assert_eq!(timer.advance(Duration::from_millis(40)), 0);
    ///     timer.pause();
    ///     timer.pause();
    ///     assert_eq!(timer.advance(Duration::from_secs(5)), 0);
    ///     assert_eq!(timer.elapsed(), Duration::from_millis(40));
    ///     timer.resume();
    ///     timer.resume();
    ///     assert_eq!(timer.advance(Duration::from_millis(60)), 1);
    ///     assert_eq!(timer.elapsed(), Duration::ZERO);
    ///     Ok(())
    /// }
    /// ```
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resumes the timer, preserving progress and completion.
    ///
    /// Idempotent: resuming a running timer changes nothing, and resuming a completed one-shot
    /// does not rearm it.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Returns whether the timer is paused, without advancing or consuming anything.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    fn advance_once(&mut self, delta: Duration) -> u128 {
        if self.elapsed == self.duration {
            return 0;
        }

        // The state invariant is elapsed < duration here, so the subtraction always succeeds.
        let remaining = self
            .duration
            .checked_sub(self.elapsed)
            .unwrap_or(Duration::ZERO);
        if delta >= remaining {
            self.elapsed = self.duration;
            1
        } else {
            // delta < remaining <= duration, so this addition cannot overflow.
            self.elapsed += delta;
            0
        }
    }

    fn advance_repeating(&mut self, delta: Duration) -> u128 {
        let period = self.duration.as_nanos();
        // period > 0 and elapsed < period; each operand is at most Duration::MAX.as_nanos()
        // (~1.8e28), so the sum fits comfortably in u128.
        let sum = self.elapsed.as_nanos() + delta.as_nanos();
        let crossings = sum / period;
        let remainder = sum % period;
        self.elapsed = duration_from_nanos(remainder);
        crossings
    }
}

/// Rebuilds a `Duration` from an exact nanosecond count below `Duration::MAX.as_nanos()`.
fn duration_from_nanos(nanos: u128) -> Duration {
    const NANOS_PER_SEC: u128 = 1_000_000_000;
    let secs = nanos / NANOS_PER_SEC;
    let subsec_nanos = nanos % NANOS_PER_SEC;
    // The caller's invariant bounds nanos by Duration::MAX.as_nanos(), so both conversions fit.
    let secs = u64::try_from(secs).unwrap_or(u64::MAX);
    let subsec_nanos = u32::try_from(subsec_nanos).unwrap_or(u32::MAX);
    Duration::new(secs, subsec_nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn timer(duration: Duration, mode: TimerMode) -> Timer {
        match Timer::new(duration, mode) {
            Ok(timer) => timer,
            Err(_) => unreachable!("positive durations construct successfully"),
        }
    }

    #[test]
    fn zero_duration_is_rejected_in_both_modes() {
        for mode in [TimerMode::Once, TimerMode::Repeating] {
            let result = Timer::new(Duration::ZERO, mode);
            assert!(matches!(
                result,
                Err(error) if matches!(error.kind(),
                    VMNLErrorKind::InvalidState(message)
                    if message == "timer duration must be strictly positive")
            ));
        }
    }

    #[test]
    fn new_timer_starts_unpaused_with_full_remaining() {
        let timer = timer(100 * MS, TimerMode::Once);
        assert!(!timer.is_paused());
        assert!(!timer.is_finished());
        assert_eq!(timer.elapsed(), Duration::ZERO);
        assert_eq!(timer.remaining(), 100 * MS);
        assert_eq!(timer.duration(), 100 * MS);
        assert_eq!(timer.mode(), TimerMode::Once);
    }

    #[test]
    fn once_counts_a_single_boundary_and_sticks() {
        let mut timer = timer(100 * MS, TimerMode::Once);
        assert_eq!(timer.advance(99 * MS), 0);
        assert_eq!(timer.advance(MS), 1);
        assert!(timer.is_finished());
        assert_eq!(timer.elapsed(), 100 * MS);
        assert_eq!(timer.remaining(), Duration::ZERO);
        assert_eq!(timer.advance(350 * MS), 0);
        assert_eq!(timer.advance(Duration::ZERO), 0);
        assert_eq!(timer.elapsed(), 100 * MS);
    }

    #[test]
    fn once_discards_overshoot_but_clamps_elapsed() {
        let mut timer = timer(100 * MS, TimerMode::Once);
        assert_eq!(timer.advance(350 * MS), 1);
        assert_eq!(timer.elapsed(), 100 * MS);
        assert!(timer.is_finished());
    }

    #[test]
    fn repeating_preserves_remainder_across_calls() {
        let mut timer = timer(100 * MS, TimerMode::Repeating);
        assert_eq!(timer.advance(350 * MS), 3);
        assert_eq!(timer.elapsed(), 50 * MS);
        assert_eq!(timer.remaining(), 50 * MS);
        assert!(!timer.is_finished());
        assert_eq!(timer.advance(50 * MS), 1);
        assert_eq!(timer.elapsed(), Duration::ZERO);
        assert_eq!(timer.remaining(), 100 * MS);
    }

    #[test]
    fn repeating_partition_equivalence_holds() {
        let mut split = timer(100 * MS, TimerMode::Repeating);
        assert_eq!(split.advance(40 * MS), 0);
        assert_eq!(split.advance(30 * MS), 0);
        assert_eq!(split.advance(80 * MS), 1);
        assert_eq!(split.elapsed(), 50 * MS);

        let mut single = timer(100 * MS, TimerMode::Repeating);
        assert_eq!(single.advance(150 * MS), 1);
        assert_eq!(single.elapsed(), 50 * MS);
    }

    #[test]
    fn zero_delta_never_progresses_or_fires() {
        for mode in [TimerMode::Once, TimerMode::Repeating] {
            let mut timer = timer(100 * MS, mode);
            assert_eq!(timer.advance(Duration::ZERO), 0);
            assert_eq!(timer.elapsed(), Duration::ZERO);
            assert!(!timer.is_finished());
        }
    }

    #[test]
    fn paused_timer_ignores_supplied_deltas() {
        let mut timer = timer(100 * MS, TimerMode::Repeating);
        assert_eq!(timer.advance(40 * MS), 0);
        timer.pause();
        timer.pause();
        assert!(timer.is_paused());
        assert_eq!(timer.advance(Duration::from_secs(5)), 0);
        assert_eq!(timer.elapsed(), 40 * MS);
        timer.resume();
        timer.resume();
        assert!(!timer.is_paused());
        assert_eq!(timer.advance(60 * MS), 1);
        assert_eq!(timer.elapsed(), Duration::ZERO);
    }

    #[test]
    fn restart_rearms_from_any_state_without_changing_configuration() {
        let mut timer = timer(100 * MS, TimerMode::Once);
        assert_eq!(timer.advance(150 * MS), 1);
        timer.pause();
        timer.restart();
        assert!(!timer.is_paused());
        assert!(!timer.is_finished());
        assert_eq!(timer.elapsed(), Duration::ZERO);
        assert_eq!(timer.remaining(), 100 * MS);
        assert_eq!(timer.duration(), 100 * MS);
        assert_eq!(timer.mode(), TimerMode::Once);
        assert_eq!(timer.advance(Duration::ZERO), 0);
        assert!(!timer.is_finished());
    }

    #[test]
    fn repeating_one_nanosecond_period_counts_max_delta_exactly() {
        let mut timer = timer(Duration::from_nanos(1), TimerMode::Repeating);
        assert_eq!(timer.advance(Duration::MAX), Duration::MAX.as_nanos());
        assert_eq!(timer.elapsed(), Duration::ZERO);
    }

    #[test]
    fn repeating_max_period_does_not_saturate_the_intermediate_sum() {
        let mut timer = Timer {
            duration: Duration::MAX,
            mode: TimerMode::Repeating,
            elapsed: Duration::MAX.saturating_sub(Duration::from_nanos(1)),
            paused: false,
        };
        assert_eq!(timer.advance(Duration::MAX), 1);
        assert_eq!(
            timer.elapsed(),
            Duration::MAX.saturating_sub(Duration::from_nanos(1))
        );
    }

    #[test]
    fn once_max_period_and_delta_do_not_overflow() {
        let mut timer = Timer {
            duration: Duration::MAX,
            mode: TimerMode::Once,
            elapsed: Duration::MAX.saturating_sub(Duration::from_nanos(1)),
            paused: false,
        };
        assert_eq!(timer.advance(Duration::MAX), 1);
        assert_eq!(timer.elapsed(), Duration::MAX);
        assert!(timer.is_finished());
    }

    #[test]
    fn timers_are_independent() {
        let mut first = timer(100 * MS, TimerMode::Once);
        let mut second = timer(100 * MS, TimerMode::Repeating);
        assert_eq!(first.advance(150 * MS), 1);
        second.pause();
        assert!(first.is_finished());
        assert!(!second.is_finished());
        assert!(second.is_paused());
        assert_eq!(second.advance(10 * MS), 0);
        assert_eq!(second.elapsed(), Duration::ZERO);
    }
}
