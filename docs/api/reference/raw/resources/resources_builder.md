# `ResourcesBuilder`

## Public path and maturity

Import path: `vmnl_graphics::raw::ResourcesBuilder`. Status: uniform buffers are operational in 0.1.0; combined and separate image/sampler bindings using default or explicit texture views are added for 0.2.0.

## Purpose and use cases

Binds typed `Uniform` and `FrameUniform` buffers, a texture view, and `Sampler` resources to reflected descriptor set/binding positions. Use `sampled_texture` for a combined descriptor with the default view, `sampled_texture_view` for a combined descriptor with an explicit view, or `texture`/`texture_view` and `sampler` for separate scalar descriptors.

## Public API

`uniform(set, binding, &Uniform<TData>)`, `frame_uniform(set, binding, &FrameUniform<TData>)`, `sampled_texture(set, binding, &Texture2D, &Sampler)`, `sampled_texture_view(set, binding, &TextureView2D, &Sampler)`, `texture(set, binding, &Texture2D)`, `texture_view(set, binding, &TextureView2D)`, `sampler(set, binding, &Sampler)`, and `build(&Context) -> VMNLResult<Resources>`.

## Construction, defaults, and validation

Created for a required pipeline. Reusing the same set/binding is remembered as an error. Build requires the same `Context` identity as the pipeline and every resource; supplied/required sets and bindings must match. Sampled images must include sampled-image usage, and their samplers must pass context, filter, border-color, and view-mapping compatibility checks. `sampled_texture` binds the texture's default `TextureView2D`; `sampled_texture_view` binds the supplied view. Both require a scalar `CombinedImageSampler` descriptor. `texture` binds the default view and `texture_view` binds the supplied view to a scalar `SampledImage` descriptor; `sampler` binds a `Sampler` to a scalar `Sampler` descriptor. Explicit views retain the same image storage and upload dependency as their texture. VMNL reflects shader image/sampler pairs from SPIR-V sampling operations, including resource values passed through helper functions, and validates each pair during build. One image may be paired with multiple views and samplers. Texel-coordinate samplers require explicit, statically known LOD zero and reject shader operations that violate unnormalized-coordinate restrictions; normalized-coordinate samplers keep shader-selected LOD behavior. Shader depth comparison and samplerless `texelFetch` are unsupported. Cross-stage descriptor conflicts and arrays, including length-one arrays, return `InvalidState`; unsupported types and push constants remain rejected. If frame uniforms are bound, their swapchain image counts must match.

Required bindings and every supplied buffer's device are validated during `build`, including when `Uniform` and `FrameUniform` bindings are mixed. Only frame-uniform descriptor set allocation is deferred until frame recording.

## Units, coordinates, and valid ranges

Set/binding numbers are shader-declared `u32` indices.

## Ownership, lifecycle, and threading

Builder stores cloned buffer, image-view and sampler handles, shared frame-uniform slots, and shared upload-completion state; it is consumed by build. `ResourcesBuilder` implements `Send + Sync`, so the handle can be moved or shared between Rust threads. This does not make `Texture2D`, `Sampler`, `Context`, or `Window` thread-safe; building or using resources still requires the owning context and window on their permitted threads. `Resources` remains tied to pipeline layout/device but does not borrow the pipeline, uniform values, texture, or sampler.

## Errors, panics, and failure conditions

Returns `InvalidState` with set/binding detail for contract mismatches, including an incompatible view/sampler pair or missing sampled usage; `VulkanValidationFailed` for conversion/internal validation; or `VulkanDescriptorSetCreationFailed` for static descriptor set allocation. Shader compilation and resource validation remain separate pipeline-build and resource-build errors. `FrameUniform` descriptor set allocation happens later during frame recording.

## Allocation, transfers, synchronization, and GPU cost

Binding updates ordered CPU maps. Static resources allocate Vulkan descriptor sets at build. `FrameUniform` bindings allocate descriptor sets during frame recording so the descriptor points at the current slot for the acquired swapchain image. A texture's pending initial upload is joined into each consuming frame's GPU dependency chain without a host wait; repeated resources in one frame share one upload dependency. No quantitative performance guarantee is specified.

## Platform, Vulkan, and display constraints

Subject to Vulkan descriptor limits and the current restricted raw layout model. The texture image must support sampled usage and the selected sampler settings. Default and explicit bindings use single-layer, single-mip 2D views; explicit views currently select only a compatible RGBA8 UNORM/sRGB format and component mapping. Descriptor arrays, storage resources, mip-range selection, and push constants are not supported.

## Example and related types

```rust,no_run
# extern crate vmnl_graphics;
# use vmnl_graphics::{Context, ShaderSource, Window};
# use vmnl_graphics::raw::{Pipeline, Pod, Resources, Uniform, Vertex, Zeroable};
# #[repr(C)] #[derive(Clone, Copy, Pod, Zeroable, Vertex)]
# struct V { #[format(R32G32_SFLOAT)] position: [f32; 2] }
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Data { color: [f32; 4] }

# fn main() -> vmnl_graphics::VMNLResult<()> {
# let context = Context::new()?;
# let window = Window::new(&context)?;
# let pipeline = Pipeline::<V>::builder()
#   .vertex_shader(ShaderSource::Path("shader.vert".into()))
#   .fragment_shader(ShaderSource::Path("shader.frag".into()))
#   .build(&window)?;
let uniform = Uniform::builder(Data { color: [1.0; 4] }).build(&context)?;
let resources = Resources::builder(&pipeline)
    .uniform(0, 0, &uniform)
    .build(&context)?;
# drop(resources);
# Ok(())
# }
```

Related: [`Resources`](resources.md), [`Uniform`](../uniforms/uniform.md), [`FrameUniform`](../uniforms/frame_uniform.md), and [`Pipeline`](../pipeline/pipeline.md).
