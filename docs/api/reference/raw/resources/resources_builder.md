# `ResourcesBuilder`

## Public path and maturity

Import path: `vmnl_graphics::raw::ResourcesBuilder`. Status: uniform buffers are operational in 0.1.0; combined image samplers using a texture's default view are added for 0.2.0.

## Purpose and use cases

Binds typed `Uniform` and `FrameUniform` buffers, or a `Texture2D` default view with a `Sampler`, to reflected descriptor set/binding positions.

## Public API

`uniform(set, binding, &Uniform<TData>)`, `frame_uniform(set, binding, &FrameUniform<TData>)`, `sampled_texture(set, binding, &Texture2D, &Sampler)`, and `build(&Context) -> VMNLResult<Resources>`.

## Construction, defaults, and validation

Created for a required pipeline. Reusing the same set/binding is remembered as an error. Build requires the same `Context` identity as the pipeline and every resource; supplied/required sets and bindings must match. A sampled texture must include sampled-image usage, and its sampler must pass the texture view's context, filter, and border-color compatibility checks. `sampled_texture` binds the texture's default `TextureView2D` and requires a scalar `CombinedImageSampler` shader descriptor. If frame uniforms are bound, their swapchain image counts must match. Only scalar `UniformBuffer` and `CombinedImageSampler` descriptors are supported; missing, extra, duplicate, array, unsupported-type, and push-constant contracts are rejected.

Required bindings and every supplied buffer's device are validated during `build`, including when `Uniform` and `FrameUniform` bindings are mixed. Only frame-uniform descriptor set allocation is deferred until frame recording.

## Units, coordinates, and valid ranges

Set/binding numbers are shader-declared `u32` indices.

## Ownership, lifecycle, and threading

Builder stores cloned buffer, image-view and sampler handles, shared frame-uniform slots, and shared upload-completion state; it is consumed by build. `ResourcesBuilder` implements `Send + Sync`, so the handle can be moved or shared between Rust threads. This does not make `Texture2D`, `Sampler`, `Context`, or `Window` thread-safe; building or using resources still requires the owning context and window on their permitted threads. `Resources` remains tied to pipeline layout/device but does not borrow the pipeline, uniform values, texture, or sampler.

## Errors, panics, and failure conditions

Returns `InvalidState` with set/binding detail for contract mismatches, including an incompatible sampler or missing sampled usage; `VulkanValidationFailed` for conversion/internal validation; or `VulkanDescriptorSetCreationFailed` for static descriptor set allocation. Shader compilation and resource validation remain separate pipeline-build and resource-build errors. `FrameUniform` descriptor set allocation happens later during frame recording.

## Allocation, transfers, synchronization, and GPU cost

Binding updates ordered CPU maps. Static resources allocate Vulkan descriptor sets at build. `FrameUniform` bindings allocate descriptor sets during frame recording so the descriptor points at the current slot for the acquired swapchain image. A texture's pending initial upload is joined into each consuming frame's GPU dependency chain without a host wait; repeated resources in one frame share one upload dependency. No quantitative performance guarantee is specified.

## Platform, Vulkan, and display constraints

Subject to Vulkan descriptor limits and the current restricted raw layout model. The texture image must support sampled usage and the selected sampler settings. This method binds only the default 2D view; separate image/sampler bindings and explicit texture-view bindings are not yet supported.

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
