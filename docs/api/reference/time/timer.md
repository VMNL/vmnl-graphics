# `Timer`

## Public path and maturity

Import path: `vmnl_graphics::time::Timer`. Status: accepted for 0.2.0; not part of the published
0.1.0 baseline.

## Purpose and use cases

Duration-driven one-shot and repeating timers for animation periods, cooldowns, and delayed
application actions. A timer accumulates a `Duration` supplied by the caller through `advance`; it
never reads a wall/monotonic clock, owns no loop, triggers no callback, and schedules no thread.
The same delta can advance multiple timers, and clients may supply real-time, paused, scaled,
fixed-step, or synthetic deltas. Use [`Clock`](clock.md) to measure a delta, a `Timer` to consume
the application's selected delta; a timer's pause state is independent of any clock and of other
timers.

## Public API

`Timer::new(duration, mode) -> VMNLResult<Timer>`, `advance(&mut self, delta) -> u128`,
`duration()`, `mode()`, `elapsed()`, `remaining()`, `is_finished()`, `restart()`, `pause()`,
`resume()`, `is_paused()`. State is private; `Timer` is not `Copy`/`Clone` and has no `Default`.
Durations use `vmnl_graphics::time::Duration` (the standard `std::time::Duration` re-export).

## Construction, defaults, and validation

`new` requires a strictly positive duration for both modes (`Duration::MAX` is valid) and an
explicit [`TimerMode`](timer_mode.md); zero is rejected with `VMNLErrorKind::InvalidState` and the
message `timer duration must be strictly positive`. Successful construction starts unpaused with
elapsed zero, full remaining, and `is_finished() == false`. Period and mode are immutable;
construct a replacement timer to change them.

## Units, coordinates, and valid ranges

Nanosecond-exact `Duration` arithmetic. `elapsed` is `Once`: `[0, duration]`, `Repeating`:
current cycle `[0, duration)`. `remaining` is `duration - elapsed`: zero after one-shot
completion, `(0, duration]` for repeating (an exact boundary starts a new full period).
`advance` returns the exact count of boundaries crossed by that call as `u128`; a one-shot clamps
at its duration and discards overshoot, a repeating timer preserves the fractional remainder.

## Ownership, lifecycle, and threading

An owned value; `Send + Sync` through the standard types it stores. Mutation follows ordinary Rust
borrowing (`&mut self`). Each timer is independent of every other timer and of any clock.

## Errors, panics, and failure conditions

Only construction can fail (zero duration, see above). `advance` and all getters are infallible and
never panic; arithmetic uses `u128` nanoseconds internally, so `Duration::MAX` periods and deltas
cannot overflow or wrap.

## Allocation, transfers, synchronization, and GPU cost

Successful construction and updates perform bounded integer arithmetic independent of the number
of crossed periods, with no heap allocation, lock, worker, sleep, OS clock read, GPU submission, or
backend initialization. Only the rejected zero-duration construction may allocate the structured
error's message.

## Platform, Vulkan, and display constraints

None: timers run headless, without a `Context`, window, display, or GPU. Two timers advanced with
the same delta sequence from the same configuration produce the same observations.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::time::{Clock, Duration, Timer, TimerMode};

# fn main() -> vmnl_graphics::VMNLResult<()> {
let mut frame_clock = Clock::new();
let mut animation = Timer::new(Duration::from_millis(120), TimerMode::Repeating)?;
let mut cooldown = Timer::new(Duration::from_secs(2), TimerMode::Once)?;

// Once per application update, share the same selected delta:
let dt = frame_clock.restart();
let animation_crossings = animation.advance(dt);
let cooldown_crossings = cooldown.advance(dt);
let _observations = (animation_crossings, cooldown_crossings);
# Ok(())
# }
```

Related: [measure elapsed time](../../workflows/measure_elapsed_time.md) workflow,
[`TimerMode`](timer_mode.md), and [`Clock`](clock.md).
