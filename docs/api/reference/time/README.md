# Time

Elapsed-time measurement primitives. Neither item requires a `Context`, a window, GLFW
initialization, or a Vulkan device.

| Item | Role |
|---|---|
| [`Clock`](clock.md) | Owned monotonic elapsed-time measurement with explicit pause/resume/restart |
| `Duration` | Direct re-export of [`std::time::Duration`](https://doc.rust-lang.org/std/time/struct.Duration.html); nonnegative duration values constructible without a clock |
