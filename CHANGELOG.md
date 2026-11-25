# Changelog

Notable user-facing changes are recorded here.

## Unreleased

### Changed

- Circle-family width setters now preserve the fill mode. Use `.filled(false)` explicitly
  when an outline is intended after selecting a filled shape. The default remains unfilled.

### Added

- Circle builder getters expose the selected fill mode and stored inner/outer offsets.
- `vmnl_graphics::time::{Clock, Duration}`: an owned elapsed-time `Clock` for independent
  measurements such as frame deltas, with explicit `pause`/`resume`/`restart`, backed by
  `std::time::Instant` and re-exporting the standard `Duration` unchanged. It runs without
  a `Context`, window, GLFW initialization, or GPU.
- `vmnl_graphics::time::{Timer, TimerMode}`: one-shot (`Once`) and repeating timers advanced
  explicitly by a caller-supplied `Duration` via `advance`, which returns the exact number of
  period boundaries crossed. Timers read no clock and run without a `Context`, window, or GPU.

### Fixed

- Frame submission returns `VulkanOutOfDate` for an out-of-date presentation instead of
  logging the condition and reporting success.
- Frame submission returns `VulkanDeviceLost` when presentation or its follow-up fence
  submission reports device loss after a successful draw submission.
- Consecutive raw frames can sample the same texture while its initial upload is pending.

## 0.1.0 - 2026-11-12

Initial release of VMNL Graphics for Vulkan rendering, windowing, and input.

### Added

- Standalone `vmnl_graphics` crate, with procedural derives re-exported
  through its public API.
- Native windows, events, monitors, keyboard, mouse, and cursor controls.
- Joystick/gamepad sampling, per-window button transitions, and configurable
  stick processing.
- 2D shapes: rectangles with optional rounded corners, triangles, polygons,
  lines, polylines, circles, ellipses, rings, sectors, and arcs.
- Custom indexed geometry and shaders.
- Raw typed pipelines, geometry, uniform buffers, and ordered composition
  with 2D passes.
- Optional window depth attachments and raw pipeline depth testing.
- Runnable examples, Rustdoc, and a dedicated public API book.

### Requirements

- Rust 1.95 or later.
- Native build dependencies and a Vulkan-capable runtime as documented
  in the build guide.

### Known limitations

- Textures, text rendering, and optimized batching are unavailable.
- Some raw traits remain coupled to Vulkano; this interoperability is unstable.
- Native platform and physical-controller qualification remains incomplete.
  See the platform support documentation for the exact evidence boundaries.
