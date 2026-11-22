# `Resources`

## Public path and maturity

Import path: `vmnl_graphics::raw::Resources`. Status: uniform-buffer resources are operational in 0.1.0; combined and separate image/sampler resources using a texture's default view are added for 0.2.0.

## Purpose and use cases

Owns descriptor resources compatible with one raw pipeline layout for `draw_raw_2d_with`. Resources can be static, or frame-varying when they bind a `FrameUniform`.

## Public API

`Resources::builder(&Pipeline<TVertex>) -> ResourcesBuilder`. Fields are private; no clone/default/accessors.

## Construction, defaults, and validation

Builder captures the pipeline device/layout. Every required supported binding must be supplied before build. Supported bindings are scalar uniform buffers, combined image samplers from `Texture2D` or `TextureView2D` plus `Sampler`, and separate sampled-image and sampler descriptors. Shader image/sampler pairs are reflected from SPIR-V sampling instructions, including helper-function resource arguments; an image may pair with multiple views and samplers. If any binding uses `FrameUniform`, every `FrameUniform` in the same `Resources` must have the same swapchain image count. Texture, view and sampler handles must share the pipeline's `Context` identity and satisfy sampled usage and shader/sampler compatibility.

## Units, coordinates, and valid ranges

Descriptor set/binding indices are `u32` and must match shader declarations. Descriptor arrays are not supported, including length-one arrays. Texel-coordinate samplers require explicit, statically verifiable LOD zero; implicit or other LOD operations and Vulkan-incompatible sampling operations are rejected. Shader depth comparison is unsupported.

## Ownership, lifecycle, and threading

Owns shared descriptor/device/layout state; borrowed during frame recording. It is logically tied to its pipeline layout/device. Frame-uniform resources are also tied to the swapchain image count observed when their frame uniforms were built. `Resources` implements `Send + Sync`, so the handle can be moved or shared between Rust threads. This does not make `Context` or `Window` thread-safe or move GPU operations away from their owning thread.

## Errors, panics, and failure conditions

Builder entry is infallible; resource build/submission can reject device, layout, binding, sampled-usage, sampler, descriptor allocation, or swapchain image-count mismatches.

## Allocation, transfers, synchronization, and GPU cost

Static resources allocate descriptor sets at build. `FrameUniform` resources keep shared frame-uniform slots and allocate descriptor sets during frame recording so the acquired image uses the latest slot. Texture resources retain their selected image view, sampler, and upload dependency; a pending initial upload joins each consuming frame without a host wait. Exact cost is unspecified.

## Platform, Vulkan, and display constraints

Current surface supports scalar uniform-buffer, combined image-sampler, sampled-image, and sampler descriptors. Texture resources use full-range, single-layer, single-mip 2D views; explicit views can select a compatible RGBA8 UNORM/sRGB format and component mapping on the same storage. Arrays, storage buffers, mip-range selection, and push constants remain unsupported.

## Example and related types

See [`ResourcesBuilder`](resources_builder.md), the [uniform-binding workflow](../../../workflows/bind_raw_uniforms.md), and the [raw texture-sampling workflow](../../../workflows/sample_raw_texture.md).
