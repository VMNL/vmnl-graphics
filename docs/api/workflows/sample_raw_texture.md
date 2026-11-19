# Sample a texture in a raw pipeline

Use `ResourcesBuilder::sampled_texture` for a scalar combined image-sampler
descriptor with the texture's default view. Use `sampled_texture_view` when the
shader should sample an explicit view of the same storage. To choose image and
sampler bindings independently, declare a GLSL `texture2D` and one or more
`sampler` descriptors; bind the default or explicit view with `texture` or
`texture_view`, then bind samplers with `sampler`.

```rust,no_run
# extern crate vmnl_graphics;
# use vmnl_graphics::{Context, ShaderSource, Window};
# use vmnl_graphics::raw::{self, Pipeline};
# use vmnl_graphics::texture::{PixelSource, Sampler, SamplerConfig, Texture2D, TextureFormat};
# #[repr(C)] #[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
# struct Vertex { #[format(R32G32_SFLOAT)] position: [f32; 2], #[format(R32G32_SFLOAT)] uv: [f32; 2] }
# fn main() -> vmnl_graphics::VMNLResult<()> {
# let context = Context::new()?;
# let mut window = Window::new(&context)?;
# let pipeline = Pipeline::<Vertex>::builder()
#     .vertex_shader(ShaderSource::Path("texture.vert".into()))
#     .fragment_shader(ShaderSource::Path("texture.frag".into()))
#     .build(&window)?;
# let geometry = raw::Geometry::builder([
#     Vertex { position: [-1.0, -1.0], uv: [0.0, 0.0] },
#     Vertex { position: [1.0, -1.0], uv: [1.0, 0.0] },
#     Vertex { position: [1.0, 1.0], uv: [1.0, 1.0] },
# ]).build(&context)?;
let rgba = [
    255, 0, 0, 255, 0, 255, 0, 255,
    0, 0, 255, 255, 255, 255, 255, 255,
];
let texture = Texture2D::from_pixels(
    &context,
    PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &rgba)?,
)
.build()?;
let sampler = Sampler::builder(&context)
    .config(SamplerConfig::nearest())
    .build()?;
let resources = raw::Resources::builder(&pipeline)
    .sampled_texture(0, 0, &texture, &sampler)
    .build(&context)?;

window
    .render()
    .draw_raw_2d_with(&pipeline, &resources, [&geometry])
    .submit()?;
# Ok(())
# }
```

The image must include sampled usage. The pipeline, texture and sampler must
share one `Context` identity; cloned handles of that context share the identity.
Missing, duplicate, extra, wrong-type, or incompatible bindings return
`InvalidState` with set/binding detail. Descriptor arrays and separate image or
sampler bindings are not supported by `sampled_texture`.

For separate bindings, the shader can pair one image with different samplers,
including from different descriptor sets. VMNL derives the actual image/sampler
pairs from SPIR-V sampling operations, including resource arguments passed to
shader helper functions, then validates each pair during `ResourcesBuilder::build`.
Every declared used descriptor must still be bound to the matching resource
type; descriptor arrays are rejected, including arrays of length one. Conflicting
descriptor declarations between shader stages return `InvalidState` before
Vulkano merges the layout.

Normalized-coordinate samplers allow the shader to select its sampling operation
and LOD. Texel-coordinate samplers require a single-layer, single-mip 2D image
and explicit, statically known LOD zero, such as `textureLod(..., 0.0)`. Implicit
or dynamic/nonzero LOD, depth comparison, projection, gather, bias, gradients,
and offsets are rejected for texel-coordinate sampling. These checks happen
when resources are built, before a draw is submitted.
Samplerless `texelFetch` is outside this sampling contract and returns
`InvalidState` with the image set/binding when SPIR-V identifies it.

```rust,no_run
# extern crate vmnl_graphics;
# use vmnl_graphics::raw;
# use vmnl_graphics::texture::{Sampler, Texture2D};
# use vmnl_graphics::Context;
# fn bind_separate_image_and_sampler<TVertex>(
#     context: &Context,
#     pipeline: &raw::Pipeline<TVertex>,
#     image: &Texture2D,
#     sampler: &Sampler,
# ) -> vmnl_graphics::VMNLResult<raw::Resources> {
let resources = raw::Resources::builder(pipeline)
    .texture(0, 1, image)
    .sampler(2, 0, sampler)
    .build(context)?;
# Ok(resources)
# }
```

The matching shader declarations are `layout(set = 0, binding = 1) uniform
texture2D image;` and `layout(set = 2, binding = 0) uniform sampler state;`;
sampling combines them with `sampler2D(image, state)`.

An explicit view can remap sampled channels or select the compatible RGBA8
UNORM/sRGB format if mutable-format views were requested when the image was
created. A non-identity mapping or changed view format has no inherited color
and alpha interpretation for managed-color consumers unless the view builder
declares one. Raw shader values remain shader-owned; sRGB view sampling still
performs Vulkan's format-defined RGB decode. Explicit views retain the same
image storage and upload dependency. They cover the full single-mip image; mip
range selection is outside this workflow.

`Texture2D::build` submits the initial upload without waiting. Frame submission
joins that upload dependency before the sampling draw; callers do not need to
wait on `initial_upload()` before using the texture. This is GPU ordering, not a
claim that the texture upload has completed when `build` returns.

See [`ResourcesBuilder`](../reference/raw/resources/resources_builder.md),
[`Texture2D`](../reference/texture/gpu_resources.md#texture2d-and-texture2dbuilder),
and the [raw resource contract](../reference/raw/resources/resources.md).
