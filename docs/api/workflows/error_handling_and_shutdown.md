# Handle errors and shutdown

- Propagate initialization and resource-build failures with `VMNLResult`.
- Match `VMNLErrorKind` with a wildcard because it is non-exhaustive.
- Treat `VulkanOutOfDate` from swapchain acquisition, presentation, or a zero-sized/minimized framebuffer as transient; continue processing events and retry with the same window once its framebuffer is renderable.
- Treat device-lost/incompatible-driver/init failures as requiring graphical reinitialization or orderly termination.
- Use `EventKind::Closed`, `window.close()`, and `window.is_open()` for loop termination.
- Log `error.report()` when source location is useful; do not parse its display text as a stable protocol.

```rust,no_run
# extern crate vmnl_graphics;
use vmnl_graphics::{VMNLErrorKind, VMNLResult, Window};

// Keep the event loop running after this recoverable result.
fn submit(window: &mut Window) -> VMNLResult<()> {
    match window.render().submit() {
        Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => Ok(()),
        result => result,
    }
}
```

`VulkanOutOfDate` from presentation can be reported after work reached the queue. The error does not mean that nothing was submitted or that the window must be rebuilt; VMNL preserves synchronization and requests swapchain recreation. `Ok(())` means submission and the presentation call returned without error, not that the compositor displayed the frame.

With Vulkano 0.35.2, retiring a failed presentation future can wait for the presentation queue to become idle, so this error path can block the thread calling `submit`.

See [`VMNLErrorKind`](../reference/errors/vmnl_error_kind.md), the [errors matrix](../appendices/errors_matrix.md), and canonical environment troubleshooting in [`docs/troubleshooting.md`](../../troubleshooting.md).
