# Sample a texture in a raw pipeline

Use `ResourcesBuilder::sampled_texture` for one scalar combined image-sampler
descriptor. The operation binds the `Texture2D` default view; its shader declares
`sampler2D` at the same set and binding. The shader owns UV calculation and
sampling instructions.

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
sampler bindings are not supported by this operation.

`Texture2D::build` submits the initial upload without waiting. Frame submission
joins that upload dependency before the sampling draw; callers do not need to
wait on `initial_upload()` before using the texture. This is GPU ordering, not a
claim that the texture upload has completed when `build` returns.

See [`ResourcesBuilder`](../reference/raw/resources/resources_builder.md),
[`Texture2D`](../reference/texture/gpu_resources.md#texture2d-and-texture2dbuilder),
and the [raw resource contract](../reference/raw/resources/resources.md).
