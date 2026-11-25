// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

use vmnl_graphics::time::{Clock, Duration, Timer, TimerMode};
use vmnl_graphics::{VMNLErrorKind, VMNLResult};

#[test]
fn duration_is_the_standard_type_with_exact_identity() {
    let animation_period = Duration::from_millis(120);
    let timeout = Duration::from_secs(3);
    let total = animation_period + Duration::from_millis(80);

    assert_eq!(total, Duration::from_millis(200));
    assert!(timeout > animation_period);
    assert_eq!(animation_period.as_millis(), 120);
    let seconds: f64 = animation_period.as_secs_f64();
    assert!(seconds > 0.0);

    let standard_duration: std::time::Duration = total;
    assert_eq!(standard_duration.as_millis(), 200);
    assert_eq!(Duration::ZERO, std::time::Duration::ZERO);
}

#[test]
fn clock_new_and_default_start_running() {
    let clock = Clock::new();
    assert!(!clock.is_paused());

    let default_clock = Clock::default();
    assert!(!default_clock.is_paused());
}

#[test]
fn pause_freezes_elapsed_and_resume_continues_without_the_paused_interval() {
    let mut clock = Clock::new();

    clock.pause();
    let frozen = clock.elapsed();
    assert_eq!(clock.elapsed(), frozen);
    assert!(clock.is_paused());

    clock.pause();
    assert_eq!(clock.elapsed(), frozen);
    assert!(clock.is_paused());

    clock.resume();
    assert!(!clock.is_paused());
    let after_resume = clock.elapsed();
    assert!(after_resume >= frozen);
}

#[test]
fn restart_returns_elapsed_and_leaves_the_clock_running() {
    let mut clock = Clock::new();

    let first_dt = clock.restart();
    assert!(!clock.is_paused());

    clock.pause();
    let before_restart = clock.elapsed();
    let returned = clock.restart();
    assert_eq!(returned, before_restart);
    assert!(!clock.is_paused());

    let second_dt = clock.restart();
    assert!(second_dt >= Duration::ZERO && first_dt >= Duration::ZERO);
}

#[test]
fn clocks_are_independent_instances() {
    let mut frame_clock = Clock::new();
    let mut measurement_clock = Clock::new();

    measurement_clock.pause();
    let frozen = measurement_clock.elapsed();
    let _frame_dt = frame_clock.restart();

    assert_eq!(measurement_clock.elapsed(), frozen);
    assert!(measurement_clock.is_paused());
    assert!(!frame_clock.is_paused());
}

#[test]
fn clock_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Clock>();
}

#[test]
fn timer_rejects_zero_duration_in_both_modes() {
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
fn timer_new_starts_unpaused_with_full_remaining() -> VMNLResult<()> {
    let timer = Timer::new(Duration::from_millis(100), TimerMode::Once)?;

    assert!(!timer.is_paused());
    assert!(!timer.is_finished());
    assert_eq!(timer.elapsed(), Duration::ZERO);
    assert_eq!(timer.remaining(), Duration::from_millis(100));
    assert_eq!(timer.duration(), Duration::from_millis(100));
    assert_eq!(timer.mode(), TimerMode::Once);

    Ok(())
}

#[test]
fn once_timer_counts_one_sticky_completion() -> VMNLResult<()> {
    let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Once)?;

    assert_eq!(timer.advance(Duration::from_millis(99)), 0);
    assert_eq!(timer.advance(Duration::from_millis(1)), 1);
    assert!(timer.is_finished());
    assert_eq!(timer.elapsed(), Duration::from_millis(100));
    assert_eq!(timer.remaining(), Duration::ZERO);

    assert_eq!(timer.advance(Duration::from_millis(350)), 0);
    assert_eq!(timer.advance(Duration::ZERO), 0);
    assert!(timer.is_finished());

    timer.restart();
    assert!(!timer.is_finished());
    assert_eq!(timer.elapsed(), Duration::ZERO);
    assert_eq!(timer.advance(Duration::ZERO), 0);
    assert!(!timer.is_finished());

    Ok(())
}

#[test]
fn once_timer_discards_overshoot_after_completion() -> VMNLResult<()> {
    let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Once)?;

    assert_eq!(timer.advance(Duration::from_millis(350)), 1);
    assert_eq!(timer.elapsed(), Duration::from_millis(100));
    assert_eq!(timer.remaining(), Duration::ZERO);
    assert!(timer.is_finished());

    Ok(())
}

#[test]
fn repeating_timer_preserves_remainder_and_counts_every_period() -> VMNLResult<()> {
    let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;

    assert_eq!(timer.advance(Duration::from_millis(350)), 3);
    assert_eq!(timer.elapsed(), Duration::from_millis(50));
    assert_eq!(timer.remaining(), Duration::from_millis(50));
    assert!(!timer.is_finished());

    assert_eq!(timer.advance(Duration::from_millis(50)), 1);
    assert_eq!(timer.elapsed(), Duration::ZERO);
    assert_eq!(timer.remaining(), Duration::from_millis(100));

    Ok(())
}

#[test]
fn repeating_timer_matches_across_partitions() -> VMNLResult<()> {
    let mut split = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;
    assert_eq!(split.advance(Duration::from_millis(40)), 0);
    assert_eq!(split.advance(Duration::from_millis(30)), 0);
    assert_eq!(split.advance(Duration::from_millis(80)), 1);

    let mut single = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;
    assert_eq!(single.advance(Duration::from_millis(150)), 1);

    assert_eq!(split.elapsed(), single.elapsed());
    assert_eq!(split.remaining(), single.remaining());

    Ok(())
}

#[test]
fn timer_pause_ignores_deltas_and_resume_continues() -> VMNLResult<()> {
    let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;
    assert_eq!(timer.advance(Duration::from_millis(40)), 0);

    timer.pause();
    timer.pause();
    assert!(timer.is_paused());
    assert_eq!(timer.advance(Duration::from_secs(5)), 0);
    assert_eq!(timer.elapsed(), Duration::from_millis(40));

    timer.resume();
    timer.resume();
    assert!(!timer.is_paused());
    assert_eq!(timer.advance(Duration::from_millis(60)), 1);
    assert_eq!(timer.elapsed(), Duration::ZERO);

    Ok(())
}

#[test]
fn timer_restart_rearms_from_paused_or_finished_state() -> VMNLResult<()> {
    let mut timer = Timer::new(Duration::from_millis(100), TimerMode::Once)?;
    assert_eq!(timer.advance(Duration::from_millis(150)), 1);

    timer.pause();
    timer.restart();

    assert!(!timer.is_paused());
    assert!(!timer.is_finished());
    assert_eq!(timer.elapsed(), Duration::ZERO);
    assert_eq!(timer.remaining(), Duration::from_millis(100));
    assert_eq!(timer.duration(), Duration::from_millis(100));
    assert_eq!(timer.mode(), TimerMode::Once);

    Ok(())
}

#[test]
fn timer_extreme_durations_do_not_overflow_or_truncate() -> VMNLResult<()> {
    let mut fast = Timer::new(Duration::from_nanos(1), TimerMode::Repeating)?;
    assert_eq!(fast.advance(Duration::MAX), Duration::MAX.as_nanos());
    assert_eq!(fast.elapsed(), Duration::ZERO);

    let mut huge_repeating = Timer::new(Duration::MAX, TimerMode::Repeating)?;
    assert_eq!(
        huge_repeating.advance(Duration::MAX.saturating_sub(Duration::from_nanos(1))),
        0
    );
    assert_eq!(
        huge_repeating.advance(Duration::MAX),
        1,
        "MAX - 1ns progress plus MAX delta crosses exactly one MAX period"
    );
    assert_eq!(
        huge_repeating.elapsed(),
        Duration::MAX.saturating_sub(Duration::from_nanos(1))
    );

    let mut huge_once = Timer::new(Duration::MAX, TimerMode::Once)?;
    assert_eq!(huge_once.advance(Duration::MAX), 1);
    assert_eq!(huge_once.elapsed(), Duration::MAX);
    assert!(huge_once.is_finished());

    Ok(())
}

#[test]
fn timers_are_independent_instances() -> VMNLResult<()> {
    let mut one_shot = Timer::new(Duration::from_millis(100), TimerMode::Once)?;
    let mut repeating = Timer::new(Duration::from_millis(100), TimerMode::Repeating)?;

    assert_eq!(one_shot.advance(Duration::from_millis(150)), 1);
    repeating.pause();
    repeating.restart();

    assert!(one_shot.is_finished());
    assert!(!repeating.is_finished());
    assert!(!repeating.is_paused());
    assert_eq!(repeating.elapsed(), Duration::ZERO);

    Ok(())
}

#[test]
fn timer_mode_is_copyable_and_comparable() {
    let mode = TimerMode::Repeating;
    let copied = mode;
    assert_eq!(mode, copied);
    assert_ne!(TimerMode::Once, TimerMode::Repeating);
    assert_eq!(format!("{mode:?}"), "Repeating");
}

#[test]
fn timer_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Timer>();
}

#[test]
fn clock_and_timer_compose_with_a_shared_delta() -> VMNLResult<()> {
    let mut frame_clock = Clock::new();
    let mut animation = Timer::new(Duration::from_millis(120), TimerMode::Repeating)?;
    let mut cooldown = Timer::new(Duration::from_secs(2), TimerMode::Once)?;

    let dt = frame_clock.restart();
    let animation_crossings = animation.advance(dt);
    let cooldown_crossings = cooldown.advance(dt);

    assert_eq!(animation_crossings, 0);
    assert_eq!(cooldown_crossings, 0);
    assert!(!animation.is_finished());
    assert!(!cooldown.is_finished());

    Ok(())
}
