# Measure elapsed time

`vmnl_graphics::time` provides owned clocks and standard duration values without a `Context`,
window, GLFW initialization, or Vulkan device. The application owns its loop; sampling order and
the first-frame policy belong to the client.

## Construct duration values independently

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::time::Duration;

let animation_period = Duration::from_millis(120);
let timeout = Duration::from_secs(3);
let total = animation_period + Duration::from_millis(80);
assert_eq!(total, Duration::from_millis(200));
assert!(timeout > animation_period);

// Exact standard-library type identity, no conversion or wrapper.
let standard_duration: std::time::Duration = total;
assert_eq!(standard_duration.as_millis(), 200);
```

`Duration` is nonnegative and follows the
[standard `Duration` contract](https://doc.rust-lang.org/std/time/struct.Duration.html).

## Sample a frame delta once and distribute it

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::time::{Clock, Duration};

fn update_movement(_dt: Duration) { /* client-owned logic */ }
fn update_animation(_dt: Duration) { /* client-owned logic */ }

let mut frame_clock = Clock::new();

// At the beginning of each application frame:
let dt = frame_clock.restart();
update_movement(dt);
update_animation(dt);

// After a loading phase, restart again to exclude that phase from the next delta:
let dt = frame_clock.restart();
assert!(dt >= Duration::ZERO);
```

No frame numbering, smoothing, clamping, scaling, sleep, or fixed timestep is implicit in `Clock`.

## Use independent clocks with pause/resume

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::time::Clock;

let mut frame_clock = Clock::new();
let mut measurement_clock = Clock::new();

measurement_clock.pause();
let frozen = measurement_clock.elapsed();
let _frame_dt = frame_clock.restart();
assert_eq!(measurement_clock.elapsed(), frozen);
assert!(measurement_clock.is_paused());

measurement_clock.pause(); // Still frozen.
measurement_clock.resume(); // Continue from frozen; exclude the paused interval.
assert!(!measurement_clock.is_paused());

measurement_clock.pause();
let before_restart = measurement_clock.elapsed();
let returned = measurement_clock.restart();
assert_eq!(returned, before_restart);
assert!(!measurement_clock.is_paused());
```

Related: [`Clock` reference](../reference/time/clock.md) and the
[event loop](window_event_loop.md) workflow. GLFW's global timer
([event processing and timers](../reference/window/events/event_processing_and_timers.md)) is a
separate source; use `Clock` for independent measurements.
