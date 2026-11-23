# `FrameRenderer`

## Public path and maturity

Import path: `vmnl_graphics::FrameRenderer<'w, 'g>`; created by `Window::render()`. Status: 0.1.0; 2D and raw rendering.

## Purpose and use cases

Records ordered logical passes for one frame and submits them through the borrowed window.

## Public API

| Method | Contract |
|---|---|
| `mode(RenderMode)` | Select object submission strategy. |
| `draw2d([&D; N])` | Append a pass from `Drawable2D` values. |
| `draw_raw_2d(&Pipeline<T>, [&Geometry<T>; N])` | Append raw pass without descriptors. |
| `draw_raw_2d_with(&Pipeline<T>, &Resources, [&Geometry<T>; N])` | Append raw pass with descriptors. |
| `write_frame_uniform(&mut FrameUniform<T>, data)` | Queue one frame-uniform write for the acquired swapchain image. |
| `submit()` | Consume the builder, acquire/record/submit/present, optionally poll events. |

Passes execute in append order. Empty arrays create empty logical passes; a frame with no pass still clears and presents.

## Construction, defaults, and validation

`Window::render()` starts with `RenderMode::PerObject`, no passes, and no queued frame-uniform writes.
During submission, VMNL checks raw pipeline, geometry, and resource compatibility, viewport/scissor
validity, and bound or queued frame-uniform image counts against the current swapchain before
acquiring an image. If acquisition triggers recreation, VMNL checks the replacement swapchain
before retrying.

## Units, coordinates, and valid ranges

Pass element count is the const-generic `N`. Rendering coordinates follow each layer's shader contract.

Every raw draw applies its own viewport/scissor policies using the actual acquired image extent. Full policies follow resize; fixed rectangles remain in framebuffer pixels. The following raw draw applies its own two policies, and a following 2D pass restores the full viewport and unclipped 2D pipeline state. No custom raw viewport/scissor leaks into subsequent draws.

## Ownership, lifecycle, and threading

The builder mutably borrows `Window` and borrows draw resources for `'g`; those values must outlive submission. Setters consume and return the builder; `submit` consumes it.

## Errors, panics, and failure conditions

`submit` can fail for incompatible devices/layouts/render passes, incompatible frame-uniform image counts, frame-uniform allocation/write/binding, zero-size/out-of-date swapchains, command recording, acquisition, device loss, queue submission, or presentation.

Before acquiring a swapchain image, `submit` checks each distinct initial texture upload referenced
by a raw draw. It returns `TextureUploadFailed` if the upload has a recorded failure or its fence
status query fails. Pending uploads join the GPU dependency chain without a host wait.

`VulkanOutOfDate` can be returned when acquisition or presentation finds the swapchain out of date, or when the framebuffer has no renderable extent. A presentation-stage error may follow queue submission; it does not mean that no work was submitted or that its resources are idle. VMNL requests swapchain recreation and preserves the synchronization state needed to keep the same window usable after it becomes renderable. Continue processing events and retry with that window.

`submit` returns `VulkanDeviceLost` when presentation or the subsequent fence submission reports device loss. Error cleanup can block the calling thread while the queue retires submitted resources.

Recreation preserves the [surface color pair](../surface_color.md). `SurfaceColorPairLost` means
the pair is absent from the current inventory. Retry if it returns or build a new window. An
allocation failure while preparing replacement framebuffers can be retried. Subsequent submits
return `SwapchainRecreationRequiresNewWindow` when native replacement failed after retiring the
swapchain.

Earlier `0.1.0` builds logged a presentation-stage out-of-date result and returned `Ok(())`. That result is now `Err(VulkanOutOfDate)`, matching the documented error contract. Callers that depended on the previous behavior must handle this transient error.

Optional depth attachments are cleared with the window's immutable `RenderTargetConfig::clear_depth_value` each frame. High-level 2D disables depth testing/writes; each raw pipeline uses its own immutable `DepthState`. Depth values are shared between ordered raw draws in the frame, including across intervening 2D draws; 2D neither tests nor changes them. Resize allocates replacement depth images/views with the same count and extent as the new swapchain images; those allocation/format/extent errors propagate from submission.

## Allocation, transfers, synchronization, and GPU cost

Recording passes allocates CPU vectors and clones shared GPU handles. Submission collects resolved
raw viewport/scissor states in draw order before acquisition. Queued frame-uniform writes are
applied after acquisition and before command recording. Frame-uniform resources allocate descriptor
sets during command recording. `submit` performs swapchain acquisition, command recording, queue
submission, synchronization, and presentation. Exact batching/performance guarantees are not specified.

With a depth attachment, `submit` waits on the acquired image's previous frame fence before reusing its depth image. This can block the calling thread but adds no device-wide wait. Color-only windows keep the existing synchronization path.

Viewport/scissor resolution is CPU-only and allocates nothing on success; recording sets two dynamic states per raw draw and restores the full viewport for each 2D pass. It does not rebuild pipelines on resize.

## Platform, Vulkan, and display constraints

Requires a ready window and renderable framebuffer. Resize can trigger swapchain/framebuffer recreation. `RenderMode::Batched` currently behaves like `PerObject`.

## Example and related types

```rust,no_run
# extern crate vmnl_graphics;
use vmnl_graphics::{Context, Window};
use vmnl_graphics::d2::Shape;

fn main() -> vmnl_graphics::VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::new(&context)?;
    let shape = Shape::rect(100.0, 50.0).build(&context)?;
    window.render().draw2d([&shape]).submit()?;
    Ok(())
}
```

Related: [`RenderMode`](render_mode.md), [`Drawable2D`](../../d2/drawable_2d.md), [`FrameUniform`](../../raw/uniforms/frame_uniform.md), and [raw pipelines](../../raw/pipeline/README.md).
