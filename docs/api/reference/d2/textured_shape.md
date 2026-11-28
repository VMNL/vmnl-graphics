# Indexed textured shapes

## Public path

Import `TexturedVertex2D`, `TexturedShapeBuilder` and `TexturedShape` from
`vmnl_graphics::d2`. `Shape::indexed_textured(vertices, indices, view)` starts the
builder. `build(&Window)` creates a shape for that window. Submit it through
`FrameRenderer::draw2d` alongside other 2D or raw passes.

## Geometry and defaults

Positions are framebuffer pixels starting at the top left. UVs are finite normalized
coordinates over the entire view. Coordinates outside the unit square follow the
sampler's address modes. The builder accepts repeated indices and degenerate
triangles. It validates triangle counts and indices before positions and UVs.

The default tint is opaque white. The default sampler uses `SamplerConfig::default()`.
Set `sampler` to reuse a normalized sampler from the same context.
`buffer_memory_preference` sets the preferred placement of both geometry buffers.

| Type | Methods and fields |
|---|---|
| `TexturedVertex2D` | `position`, `color`, `uv` |
| `TexturedShapeBuilder` | `buffer_memory_preference`, `sampler`, `tint`, `modulation_encoding`, `color_blend`, `build` |
| `TexturedShape` | `view`, `sampler_config`, `tint`, `modulation_encoding`, `color_blend_config` |

## Color and alpha

The view must declare its color encoding and alpha mode. The managed shader samples
`Rgba8Srgb` as native sRGB and `Rgba8Unorm` as linear color. A view declaration must
match the format's RGB decoding. The view continues to share the texture's existing
pixels. For premultiplied sRGB data, encode the RGB channels after multiplying in
linear light.

Vertex and tint RGB are linear coefficients by default. An explicit
`modulation_encoding(ColorEncoding::Srgb)` decodes both to linear RGB on the CPU before
interpolation. Their alpha values are always linear. If `M` is the combined modulation
of interpolated vertex color and tint, the shader emits:

| View alpha | Output RGB | Output alpha | Default blend |
|---|---|---|---|
| `Straight` | sampled RGB × `M.rgb` | sampled alpha × `M.a` | Straight source-over |
| `Premultiplied` | sampled RGB × `M.rgb` × `M.a` | sampled alpha × `M.a` | Premultiplied source-over |
| `Opaque` | sampled RGB × `M.rgb` | `M.a` | Straight source-over |

A `color_blend` override changes the blend state while keeping the selected shader
output representation. A transparent sampler border requires an alpha-bearing
interpretation like `Straight` or `Premultiplied`.

The first managed output route requires a native BGRA8 or RGBA8 sRGB attachment with
`SRGB_NON_LINEAR` presentation. Blending operates in linear light and the attachment
applies the output transfer to RGB. Existing color-only and raw shaders continue
to use their own input and output contracts. [Vulkan framebuffer blending](https://docs.vulkan.org/spec/latest/chapters/framebuffer.html#framebuffer-blending)

## Errors, costs, and ownership

An unsupported target pair returns `UnsupportedManagedColorTarget { pair }` from
`build` before managed geometry or pipeline allocation. The window retains its selected pair and
can still submit raw draws. Invalid geometry, unknown or conflicting view interpretation,
incompatible samplers, and a foreign window at submission return `InvalidState`.

Building initializes two geometry buffers, compiles the managed shaders, and creates
one pipeline and descriptor set per shape. An omitted sampler creates one sampler.
The shape retains the view and its binding. Submission joins pending uploads and
reuses those resources across frames and ordinary resize. Build and draw on the
window thread.

Run [`d2_textured_color`](../../../../examples/d2/textured_color/src/main.rs) to compare
straight and premultiplied source textures over black and white.
