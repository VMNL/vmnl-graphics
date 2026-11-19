# Texture GPU resources

## Public path and maturity

Import GPU texture resources from `vmnl_graphics::texture`. This is the initial
`0.2.0` resource foundation. Raw pipelines can sample the default `Texture2D`
view through combined or separate image/sampler descriptors; high-level 2D
sampling and customizable views remain separate API steps.

## Purpose and use cases

Build a single-layer, single-mip RGBA8 image from pixels, encoded static PNG
bytes, or a PNG path. Keep image storage shared across texture clones and views,
configure independent GPU samplers, and choose exactly when the initial upload
is submitted.

## Public API

### `Texture2D` and `Texture2DBuilder`

`Texture2D::from_pixels(&Context, PixelSource)`, `from_png(&Context, &[u8])`,
and `from_file(&Context, impl Into<PathBuf>)` return a builder. `with_png_options`
sets PNG declarations; `with_usage` replaces the source's usage request.

`prepare` reads/decodes the source, normalizes pixels, allocates the image,
staging buffer, and default view, then records a command buffer without queue
submission. `build` performs the same work and submits the upload, returning
without waiting for completion. Use `PreparedTexture2D::submit` when the queue
submission point must be explicit.

Texture getters return extent, format, declared encoding and alpha mode, exact
usage, and whether shader sampling is included. Clones share the image
allocation. `view` returns a full-range `TextureView2D`; this step does not yet
expose custom mip/layer ranges, format reinterpretation, or component swizzles.
`TextureView2D::validate_sampler` checks context identity, sampled usage, linear
filter support for the view format, and the border color's numeric class. It does
not inspect shader instructions. Raw `ResourcesBuilder::build` validates the
reflected shader image/sampler pairs and coordinate-mode compatibility.

`raw::ResourcesBuilder::sampled_texture(set, binding, &texture, &sampler)` binds
the default 2D view and sampler to a scalar combined image-sampler descriptor.
Use `texture` and `sampler` for separate scalar descriptors. Both resources and
the pipeline must use the same `Context` identity. The image must include sampled
usage. The raw shader controls its own UVs and sampling instructions; VMNL checks
the actual image/sampler pairs reflected from SPIR-V. Normalized coordinates keep
shader-selected LOD behavior. Texel coordinates require explicit statically known
LOD zero and reject unsupported operations such as implicit LOD, comparison,
projection, bias, gradients, and offsets.

`Texture2DBuilder::retirement_policy` selects `Wait` (default) or
`DeferToContext`. `Wait` waits only for the texture's outstanding initial upload
before its last owner releases the image. `DeferToContext` retains the image,
view, staging data, and upload dependency in the creating context. Call
`Context::collect_texture_retirements` to poll and release completed resources;
the context drains remaining entries at shutdown. Collection returns the number
of resources released and reports observed upload failures. It does not wait for
pending uploads, though backend error cleanup may block while retiring a failed
submission. A texture, view, and sampler from distinct `Context` instances are
incompatible even if both contexts select the same physical device. Windows made
from clones of one context share its texture resources.

### `PreparedTexture2D`

This value owns the image, default view, staging buffer, and recorded command
buffer. Dropping it does not submit or wait. `submit` consumes it and returns a
`Texture2D` with its upload ticket.

### `TextureView2D`

The default view retains its parent `Texture2D`, so the image storage outlives
the view. `texture` returns that owner; `shares_storage_with`, `extent`, and
`format` inspect the relationship and view properties.

### `UploadTicket` and `UploadWaitOutcome`

`Texture2D::initial_upload` returns a reusable status handle. `is_complete` is
non-blocking. `wait(Some(duration))` returns `TimedOut` while pending without
consuming or cancelling the operation; a later query/wait can observe the same
upload. `wait(None)` waits to completion. `Complete` is persistent across cloned
tickets. Fence errors remain persistent errors.

Finite waits poll the fence at a 1 ms cadence. They do not call Vulkano's
state-consuming timed fence wait. The final owner follows the selected texture
retirement policy; the context shutdown drain waits for resources that were
deferred and not explicitly collected.

### `Sampler` and `SamplerBuilder`

`Sampler::builder(&Context)` starts with `SamplerConfig::default`. `config`
replaces the complete request and `build` creates one Vulkan sampler; no cache
is used. `Sampler::config` returns the exact configuration used. Anisotropy
greater than `1.0` requires enabled `DeviceFeature::SamplerAnisotropy` and must
not exceed the selected device limit. Nonzero LOD bias must fit the device limit
and, on portability-subset devices, requires enabled
`DeviceFeature::SamplerMipLodBias`. Requests fail instead of being clamped.

## Construction, defaults, and validation

Automatic texture usage is `SAMPLED | TRANSFER_DESTINATION`; it never adds
`TRANSFER_SOURCE`. An exact usage is passed unchanged to image creation. The
initial upload fails before GPU allocation if exact usage omits
`TRANSFER_DESTINATION`.

Images use optimal tiling, one mip level, and device-preferred memory with
Vulkano's compatible memory fallback. Upload staging uses host sequential-write
memory. The initial image layout is `General`, which supports both sampled use
and exact transfer-only requests. Image-format support and allocation failures
are reported; no format or usage fallback is added.

## Units, coordinates, and valid ranges

Extent is `[width, height]` in pixels. Source row origin, layout and format
follow [`PixelSource`](descriptors.md#pixelsource). Texture formats are RGBA8
UNORM or sRGB.

## Ownership, lifecycle, and threading

Texture, view, sampler, context and ticket handles are single-threaded because
they retain `Context`'s `Rc` ownership. A view retains its texture storage.
Staging data remains owned by the upload ticket until completion. The upload
ticket is reusable and cloneable. Raw `Resources` and `ResourcesBuilder` retain
shareable image-view/sampler handles and upload completion state, so they can be
`Send + Sync` without making these source handles thread-safe.

## Errors, panics, and failure conditions

Pixel descriptor errors use `InvalidState`; malformed/unsupported PNGs retain
their decode categories; path I/O uses `TextureSourceReadFailed`. Vulkan image,
view, sampler, buffer, and command-buffer creation use their corresponding
Vulkan error categories. Queue submission, fence polling, or completion errors
use `TextureUploadFailed` and remain observable through the ticket.

## Allocation, transfers, synchronization, and GPU cost

`prepare` performs CPU source work and image, view, staging-buffer, and command
buffer allocation/recording, but no queue submission or wait. `build` also
submits one buffer-to-image transfer and returns asynchronously. `view` and
`initial_upload` allocate no GPU objects. Sampler build creates one Vulkan
sampler. No latency, allocation-count, cache, or throughput guarantee is
specified.

## Platform, Vulkan, and display constraints

Building any GPU texture or sampler requires a Vulkan `Context` and a compatible
device. The GPU integration tests also require a GLFW display. Image extent,
format features, memory types, sampler features and sampler limits are device
dependent.

## Example and related types

```rust,no_run
# extern crate vmnl_graphics;
use vmnl_graphics::{texture::{PixelSource, Texture2D, TextureFormat}, Context};

# fn main() -> vmnl_graphics::VMNLResult<()> {
let context = Context::new()?;
let rgba = [255, 0, 0, 255];
let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &rgba)?;
let texture = Texture2D::from_pixels(&context, source).build()?;
let upload = texture.initial_upload();
let _complete = upload.is_complete()?;
# Ok(())
# }
```

Related: [pixel descriptors](descriptors.md), [PNG decoding](png_decode.md),
[sampler configuration](sampler_config.md), and
[`DeviceFeature`](../device_feature.md).
