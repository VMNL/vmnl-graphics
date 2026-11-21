# `Clock`

## Public path and maturity

Import path: `vmnl_graphics::time::Clock`. Status: accepted for 0.2.0; not part of the published
0.1.0 baseline.

## Purpose and use cases

Multiple independent elapsed-time measurements — frame deltas, loading phases, explicitly
controlled updates — while the application keeps owning its loop. Event polling, drawing,
presenting, and every other clock leave a clock's state unchanged, and reading a clock never
schedules a callback or advances a simulation.

## Public API

Inherent methods: `new() -> Self`, `elapsed(&self) -> Duration`, `restart(&mut self) -> Duration`,
`pause(&mut self)`, `resume(&mut self)`, `is_paused(&self) -> bool`. `Default` is implemented with
exactly `new()` semantics. Duration values use `vmnl_graphics::time::Duration`, a direct re-export
of `std::time::Duration`. `Clock` is intentionally not `Copy` or `Clone`.

## Construction, defaults, and validation

`Clock::new()` and `Clock::default()` capture a starting `std::time::Instant`, start running with
zero accumulated active time; a later `elapsed()` may already be nonzero. Construction is
infallible and has no validation.

## Units, coordinates, and valid ranges

Results are nanosecond-resolution `Duration` values from a monotonic `Instant` source, in no
coordinate system. Interval arithmetic saturates: a backwards source interval contributes zero and
accumulation saturates at `Duration::MAX`; neither wraps nor panics. These are representational
limits, not a gameplay delta clamp.

## Ownership, lifecycle, and threading

`Clock` is an owned value and `Send + Sync` through the standard types it stores; there is no
thread affinity. Mutation follows ordinary Rust borrowing (`&mut self`) or external
synchronization. Each instance measures independently; restarting or pausing one clock never
affects another.

## Errors, panics, and failure conditions

All `Clock` operations are infallible. Duration arithmetic saturates as described above instead of
overflowing. `Duration` constructors keep the standard library's validation and panic behavior.

## Allocation, transfers, synchronization, and GPU cost

Construction, reads, and transitions perform no heap allocation, spawn no work, acquire no VMNL
global locks, and never block waiting for a frame. Each operation reads `Instant::now()` at most
once, and skips the read entirely when the frozen or running state alone determines the result.
No GPU work, transfer, or synchronization occurs.

## Platform, Vulkan, and display constraints

None: a `Clock` works headless, without a `Context`, window, display, GLFW initialization, or
Vulkan device. Source resolution, accuracy, monotonicity, and system-suspension treatment inherit
the [platform limits of `std::time::Instant`](https://doc.rust-lang.org/std/time/struct.Instant.html);
nanosecond duration storage does not guarantee nanosecond source accuracy.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::time::{Clock, Duration};

let mut frame_clock = Clock::new();
let mut measurement_clock = Clock::new();

measurement_clock.pause();
let frozen: Duration = measurement_clock.elapsed();
let frame_dt: Duration = frame_clock.restart();
assert_eq!(measurement_clock.elapsed(), frozen);
assert!(frame_dt >= Duration::ZERO);

measurement_clock.resume();
assert!(!measurement_clock.is_paused());
```

Related: [measure elapsed time](../../workflows/measure_elapsed_time.md) workflow, and the
`Duration` re-export in the [time module](README.md). GLFW's global time base
(`Window::get_time`/`set_time`) is a separate source that never changes a `Clock`.
