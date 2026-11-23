# Create a raw pipeline

1. Define a `#[repr(C)]` vertex type and derive `Pod`, `Zeroable`, and `Vertex`.
2. Annotate every vertex field with its Vulkan `#[format(...)]` and optional shader `#[name(...)]`.
3. Build a window first because the pipeline is bound to its device/render pass.
4. Supply both shader stages and optionally topology, blending, culling, front-face winding, polygon mode, line width, and viewport/scissor policies. `BlendMode` keeps the legacy opaque/alpha modes; use `ColorBlendConfig` to choose and edit opaque, straight-alpha, or premultiplied-alpha state. The built pipeline exposes the effective config. Require `FillModeNonSolid` / `WideLines` through `DeviceConfig` before context creation when needed; inspect `Context::line_width_limits()` to choose a valid width. Viewport/scissor need no optional feature; see [composition and resize](compose_2d_and_raw.md#inspect-viewport-and-scissor-policies).
5. Build and retain the pipeline for compatible geometry submissions.

Start with [`raw_triangle`](../../../examples/raw/triangle/src/main.rs). Use [`raw_shader_path`](../../../examples/raw/shader_path/src/main.rs) to build the two shader stages from files. Review [`PipelineSpec`](../reference/raw/pipeline/pipeline_spec.md) and [shader/layout safety](../concepts/shaders_vertex_layouts_and_safety.md).

Current layout limits: descriptor-count-one uniform buffers, combined image samplers, separate sampled images and samplers. Descriptor arrays (including length-one arrays), storage descriptors, and push constants are rejected. Cross-stage descriptor conflicts return `InvalidState` before layout merging. See [raw texture sampling](sample_raw_texture.md) for shader/sampler compatibility.

For straight-alpha fragment output, use `ColorBlendConfig::straight_alpha()`. For premultiplied output, use `premultiplied_alpha()` and make the shader write RGB already multiplied by alpha. RGB and alpha factors/operations can also be edited independently. Texture presence does not change the blend equation; it only changes how the fragment shader obtains its RGBA value. The selected render-target format must support color-attachment blending when enabled. Constant factors use the config's static RGBA blend constant; unsupported device/format combinations fail pipeline build.

`BlendMode::Alpha` remains source compatible and preserves its historical alpha equation `As * As + Ad * (1 - As)`. Use `ColorBlendConfig::straight_alpha()` when the desired alpha output is `As + Ad * (1 - As)`.

## Inspect pipeline state

- Run `just run raw_topology` and press Space to cycle primitive assembly for the same vertices.
  Points use a unit size. `just run raw_point_size` requests `LargePoints` for larger points.
- Run `just run raw_culling`. The square has two triangles with opposite windings.
  `C` cycles `None`, `Front`, `Back` and `FrontAndBack`. Both triangles are visible in
  `None`, one in `Front` or `Back`, and neither in `FrontAndBack`. With one visible,
  `F` reverses the front-face winding and swaps the visible triangle.
- Run `just run raw_wireframe` on a GPU that supports `FillModeNonSolid`. Space toggles
  fill and wireframe. The shared diagonal remains visible in wireframe.
- Run `just run raw_line_width` on a GPU with `WideLines` and a supported width above one.
  The right line is wider than the unit-width line on the left. The console prints the
  device limits and the selected width. Resize and verify the visible comparison persists.
- Run `just run raw_blend`. The left triangle is opaque. The middle uses straight-alpha
  output and the right uses premultiplied output. The last two should match over the same background.
