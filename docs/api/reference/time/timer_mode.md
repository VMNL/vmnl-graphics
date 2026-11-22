# `TimerMode`

## Public path and maturity

Import path: `vmnl_graphics::time::TimerMode`. Status: accepted for 0.2.0; not part of the
published 0.1.0 baseline.

## Purpose and use cases

Selects whether a [`Timer`](timer.md) completes once or fires periodically. Fixed at timer
construction and returned unchanged by `Timer::mode`.

## Public API

Variants `Once` and `Repeating`. Derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.

## Construction, defaults, and validation

No default: the mode is always supplied explicitly to `Timer::new`.

## Units, coordinates, and valid ranges

Not applicable; a plain mode selector.

## Ownership, lifecycle, and threading

A `Copy` value with no state; freely shareable across threads.

## Errors, panics, and failure conditions

Infallible.

## Allocation, transfers, synchronization, and GPU cost

None; comparing or copying the value performs no work beyond the value itself.

## Platform, Vulkan, and display constraints

None.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::time::{Duration, Timer, TimerMode};

# fn main() -> vmnl_graphics::VMNLResult<()> {
let cooldown = Timer::new(Duration::from_secs(2), TimerMode::Once)?;
assert_eq!(cooldown.mode(), TimerMode::Once);
# Ok(())
# }
```

Related: [`Timer`](timer.md) and [`Clock`](clock.md).
