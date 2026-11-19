# Texture GPU resources

## Public path and maturity

Import GPU texture resources from `vmnl_graphics::texture`. This is the `0.2.0`
resource foundation. Raw pipelines can sample the default or explicit
`TextureView2D` through combined or separate image/sampler descriptors. Views
can remap RGBA components and, when requested before image creation, select the
compatible RGBA8 UNORM/sRGB format. High-level 2D sampling remains a separate
API step.

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
allocation. `view` returns the full single-layer, single-mip identity view and
inherits the texture's encoding and alpha declarations.

`Texture2DBuilder::with_mutable_format_views(true)` requests at image creation
that the image support both RGBA8 UNORM and RGBA8 sRGB views. It defaults to
`false`; unsupported format/usage/extent combinations fail during `prepare`
before image allocation. Inspect the request on the builder and the resolved
creation policy on `PreparedTexture2D` or `Texture2D`. `supports_view_format`
reports whether a texture's creation policy permits a format; it does not
replace view construction or sampler validation.

`TextureView2D::builder` creates a view of the same image storage. The builder
can request an exact compatible format, an output-to-source RGBA component
mapping (including zero/one constants), and a `TextureViewInterpretation` for
managed-color consumers. The mapping and format are inspectable as requested
and effective values. The unchanged identity view reuses the existing Vulkan
view; a changed format or mapping creates one additional Vulkan image-view
object, retains the same image allocation and upload dependency, and has no
view cache. A changed format or non-identity mapping has no inherited sampled
interpretation unless one is supplied. Building an identity view does not
change the texture's color/alpha declarations. These declarations do not
transform stored bytes; an sRGB Vulkan view still applies Vulkan's native RGB
decode while sampling. Raw shaders may sample a view with no declared
interpretation because the shader owns how it uses those values.

Views cover the full image: this API does not select mip/layer ranges or create
mip levels. Partial updates and mip validity/generation remain separate
contracts. `TextureView2D::validate_sampler` checks context identity, sampled
usage, linear-filter support for the effective view format, border numeric
class, and opaque-black border compatibility with a non-identity mapping. It
does not inspect shader instructions. Raw `ResourcesBuilder::build` validates
the reflected shader image/sampler pairs and coordinate-mode compatibility.

`raw::ResourcesBuilder::sampled_texture(set, binding, &texture, &sampler)` binds
the default 2D view and sampler to a scalar combined image-sampler descriptor.
`sampled_texture_view` binds an explicit view to a combined descriptor;
`texture_view` binds it to a separate sampled-image descriptor. Use `texture`
and `sampler` for separate descriptors with the default view. Both resources and
the pipeline must use the same `Context` identity. The image must include
sampled usage. The raw shader controls its own UVs and sampling instructions;
VMNL checks the actual image/sampler pairs reflected from SPIR-V. Normalized
coordinates keep shader-selected LOD behavior. Texel coordinates require
explicit statically known LOD zero and reject unsupported operations such as
implicit LOD, comparison, projection, bias, gradients, and offsets.

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

The view retains its parent `Texture2D`, so image storage outlives the view.
Cloning a view shares its Vulkan image-view object. `texture`,
`shares_storage_with`, `extent`, and `effective_format` inspect ownership and
the resolved view. `requested_format`, `requested_component_mapping`, and
`requested_interpretation` preserve explicit inputs (`None` means the field was
inherited); the corresponding effective getters expose the selected mapping,
format, and optional sampled interpretation. `format` remains an alias for the
effective view format. `validate_sampler` rejects cross-context pairs,
unsampleable images, unsupported linear filtering, integer border colors for
normalized RGBA8, and opaque-black border sampling with a non-identity mapping.

`TextureView2DBuilder::build` returns `InvalidState` when the image creation
request, format features, or enabled portability-subset features do not support
the exact request. Failed device format-property queries return
`VulkanValidationFailed`; Vulkan view creation failures return
`VulkanImageViewCreationFailed`. A non-identity mapping on a portability-subset
device requires enabled `DeviceFeature::ImageViewFormatSwizzle`. The current
compatible format pair has the same component count; component reinterpretation
feature requirements are checked if a future supported pair differs.

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

Mutable-format view support is an independent exact image-creation request and
defaults off. Unsupported alternate sampled format, image usage, or extent
rejects preparation instead of silently creating a fixed-format image.

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
they retain `Context`'s `Rc` ownership. A view retains its texture storage and
its Vulkan image-view handle; raw resource bindings retain the view and the
same upload-completion dependency.
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
`initial_upload` allocate no GPU objects. Building an unchanged identity view
reuses the default view; each distinct requested format or mapping creates one
Vulkan image-view object and no image allocation. Sampler build creates one
Vulkan sampler. No latency or throughput guarantee is specified.

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

An explicit view can select the compatible sRGB format and remap channels:

```rust,no_run
# extern crate vmnl_graphics;
use vmnl_graphics::{
    raw,
    texture::{
        AlphaMode, ColorEncoding, PixelSource, Sampler, SamplerConfig, Texture2D,
        TextureComponent, TextureComponentMapping, TextureFormat, TextureView2D,
        TextureViewInterpretation,
    },
    Context, ShaderSource, Window,
};
# #[repr(C)] #[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
# struct V { #[format(R32G32_SFLOAT)] position: [f32; 2] }
# fn main() -> vmnl_graphics::VMNLResult<()> {
# let context = Context::new()?;
# let window = Window::new(&context)?;
# let pipeline = raw::Pipeline::<V>::builder()
#     .vertex_shader(ShaderSource::Path("texture.vert".into()))
#     .fragment_shader(ShaderSource::Path("texture.frag".into()))
#     .build(&window)?;
let rgba = [255, 0, 0, 255];
let texture = Texture2D::from_pixels(
    &context,
    PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &rgba)?,
)
.with_mutable_format_views(true)
.build()?;
let view = TextureView2D::builder(&texture)
    .format(TextureFormat::Rgba8Srgb)
    .component_mapping(TextureComponentMapping::new(
        TextureComponent::Blue,
        TextureComponent::Green,
        TextureComponent::Red,
        TextureComponent::Alpha,
    ))
    .interpretation(TextureViewInterpretation::new(
        ColorEncoding::Srgb,
        AlphaMode::Straight,
    ))
    .build()?;
let sampler = Sampler::builder(&context)
    .config(SamplerConfig::nearest())
    .build()?;
let _resources = raw::Resources::builder(&pipeline)
    .sampled_texture_view(0, 0, &view, &sampler)
    .build(&context)?;
# Ok(())
# }
```

Related: [pixel descriptors](descriptors.md), [PNG decoding](png_decode.md),
[sampler configuration](sampler_config.md), and
[`DeviceFeature`](../device_feature.md).
