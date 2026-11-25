# Time

Elapsed-time measurement and caller-advanced timing primitives. None of these items require a
`Context`, a window, GLFW initialization, or a Vulkan device.

| Item | Role |
|---|---|
| [`Clock`](clock.md) | Owned monotonic elapsed-time measurement with explicit pause/resume/restart |
| [`Timer`](timer.md) | Duration-driven one-shot/repeating timer advanced by a caller-supplied delta |
| [`TimerMode`](timer_mode.md) | `Once` / `Repeating` selection for `Timer` |
| `Duration` | Direct re-export of [`std::time::Duration`](https://doc.rust-lang.org/std/time/struct.Duration.html); nonnegative duration values constructible without a clock or timer |
