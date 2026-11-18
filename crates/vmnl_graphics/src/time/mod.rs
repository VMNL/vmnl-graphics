// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

//! Elapsed-time measurement primitives.
//!
//! [`Clock`](crate::time::Clock) is an owned monotonic clock for independent elapsed-time measurements such as frame
//! deltas or loading phases. [`Duration`](crate::time::Duration) is the standard-library duration type re-exported
//! unchanged, so duration values can be constructed and combined without a clock.
//!
//! Neither type requires a `Context`, a window, GLFW initialization, or a Vulkan device, and
//! reading or transitioning a clock never advances another clock, the event queue, or rendering.
//!
//! # Example
//!
//! Construct and manipulate duration values independently of any clock:
//!
//! ```
//! use vmnl_graphics::time::Duration;
//!
//! let animation_period = Duration::from_millis(120);
//! let timeout = Duration::from_secs(3);
//! let total = animation_period + Duration::from_millis(80);
//! assert_eq!(total, Duration::from_millis(200));
//! assert!(timeout > animation_period);
//! assert_eq!(animation_period.as_millis(), 120);
//! let seconds: f64 = animation_period.as_secs_f64();
//! assert!(seconds > 0.0);
//!
//! // Exact standard-library type identity, no conversion or wrapper.
//! let standard_duration: std::time::Duration = total;
//! assert_eq!(standard_duration.as_millis(), 200);
//! ```

mod clock;

pub use clock::Clock;

/// A nonnegative span of time.
///
/// This is a direct re-export of [`std::time::Duration`], preserving exact type identity: values
/// convert to and from `std::time::Duration` without any wrapper or conversion, and all
/// constructors, arithmetic, comparisons, and panics follow the
/// [standard `Duration` contract](https://doc.rust-lang.org/std/time/struct.Duration.html).
/// In particular, a `Duration` is nonnegative.
pub use std::time::Duration;
