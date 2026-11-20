# `ColorBlendConfig`

## Public path and maturity

Import path: `vmnl_graphics::common::ColorBlendConfig`. The type configures raw color-attachment blending.

## Purpose and use cases

Selects whether blending is enabled, independent RGB and alpha factors and operations, and the RGBA blend constant. The same state applies to raw draws with or without sampled textures; the fragment shader's RGBA output is what gets blended.

## Public API

Presets: `opaque()`, `straight_alpha()`, and `premultiplied_alpha()`. Configuration methods: `with_blend_enabled`, `with_color_blend`, `with_alpha_blend`, and `with_blend_constants`. Inspectors: `blend_enabled`, `color_source_factor`, `color_destination_factor`, `color_operation`, `alpha_source_factor`, `alpha_destination_factor`, `alpha_operation`, and `blend_constants`. Implements `Default` as `opaque()`; derives `Clone`, `Copy`, `Debug`, and `PartialEq`. Fields are private so new fields can be added compatibly.

## Construction, defaults, and validation

`opaque()` disables blending and writes the fragment output directly. `straight_alpha()` configures RGB source-over as `Cs * As + Cd * (1 - As)` and alpha as `As + Ad * (1 - As)`. `premultiplied_alpha()` configures RGB as `Cs + Cd * (1 - As)` and alpha as `As + Ad * (1 - As)`; the shader must output premultiplied RGB. Each preset can be changed through the `with_*` methods.

Pass the copied configuration to `raw::PipelineSpec::color_blend` to set all fields explicitly.
A later `blend_mode` call selects the complete legacy preset.
Use [`ColorBlendOverrides`](color_blend_overrides.md) for per-field inheritance.
`PipelineSpec::color_blend_config_value` previews the local request against the opaque preset.
`Pipeline::color_blend_config_value` reports the effective configuration used by the built pipeline.

Configuration is CPU-only and does not query the device or allocate GPU resources. Pipeline construction checks Vulkan and render-target compatibility. No fallback or automatic rewrite is performed.

## Units, coordinates, and valid ranges

Blend constants are four `f32` values in RGBA order and are passed to Vulkan unchanged. They are used only by [`ColorBlendFactor::ConstantColor`](color_blend_factor.md) and related constant factors. Factors and operations are described by [`ColorBlendFactor`](color_blend_factor.md) and [`ColorBlendOperation`](color_blend_operation.md).

## Ownership, lifecycle, and threading

The value is `Copy` and owns no GPU handle. It can be built or changed on any thread. A pipeline captures it at build; changing or dropping the original value does not modify an existing pipeline. Building a pipeline compiles shaders and creates Vulkan pipeline objects as documented by [`PipelineSpec`](../raw/pipeline/pipeline_spec.md).

## Errors, panics, and failure conditions

Selecting a value is infallible. Pipeline build can return `VulkanPipelineCreationFailed` if the selected device or target cannot support the requested state. In particular, the render-target format must support Vulkan's `VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND` when blending is enabled. On Vulkan portability-subset devices, `ConstantAlpha` and `OneMinusConstantAlpha` used as RGB factors require `constantAlphaColorBlendFactors`; VMNL does not currently expose that feature through `DeviceFeature`, so Vulkano rejects such a pipeline. Dual-source (`Source1*`) factors are not exposed because VMNL does not currently let clients request `dualSrcBlend`.

## Allocation, transfers, synchronization, and GPU cost

The config and its getters allocate nothing and perform no GPU access. The values become immutable pipeline state during pipeline creation. No per-frame command, upload, wait, or extra resource is added by this API.

## Platform, Vulkan, and display constraints

Vulkan implementations may expose different blend support for color attachment formats. The selected render-target format and device must support the requested state; pipeline build returns a structured VMNL error when Vulkano rejects it. The pipeline remains tied to its originating window and render pass.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::common::{ColorBlendConfig, ColorBlendFactor, ColorBlendOperation};
use vmnl_graphics::raw::{BlendMode, Pipeline};

let config = ColorBlendConfig::straight_alpha()
    .with_alpha_blend(
        ColorBlendFactor::One,
        ColorBlendFactor::OneMinusSourceAlpha,
        ColorBlendOperation::Add,
    )
    .with_blend_constants([0.25, 0.5, 0.75, 1.0]);
let spec = Pipeline::<[f32; 2]>::builder()
    .blend_mode(BlendMode::Alpha)
    .color_blend(config);

assert_eq!(spec.blend_mode_value(), BlendMode::Alpha);
assert_eq!(spec.color_blend_config_value(), config);
```

Related: [`ColorBlendFactor`](color_blend_factor.md), [`ColorBlendOperation`](color_blend_operation.md), [`PipelineSpec`](../raw/pipeline/pipeline_spec.md), and [`Pipeline`](../raw/pipeline/pipeline.md).
