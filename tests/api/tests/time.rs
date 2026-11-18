// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

use vmnl_graphics::time::{Clock, Duration};

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
fn elapsed_is_non_consuming_and_monotonic_while_running() {
    let clock = Clock::new();

    let first = clock.elapsed();
    let second = clock.elapsed();

    assert!(second >= first);
    assert!(!clock.is_paused());
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

    clock.resume();
    assert!(!clock.is_paused());
    assert!(clock.elapsed() >= after_resume);
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
