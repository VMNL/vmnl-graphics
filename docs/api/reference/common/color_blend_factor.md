# `ColorBlendFactor`

## Public path and maturity

Import path: `vmnl_graphics::common::ColorBlendFactor`. This is VMNL's backend-independent subset of Vulkan blend factors.

## Purpose and use cases

Selects the multiplier applied to a source or destination color before the RGB or alpha blend operation.

## Public API

Variants: `Zero`, `One`, `SourceColor`, `OneMinusSourceColor`, `DestinationColor`, `OneMinusDestinationColor`, `SourceAlpha`, `OneMinusSourceAlpha`, `DestinationAlpha`, `OneMinusDestinationAlpha`, `ConstantColor`, `OneMinusConstantColor`, `ConstantAlpha`, `OneMinusConstantAlpha`, and `SourceAlphaSaturate`. The enum is `#[non_exhaustive]` and derives `Clone`, `Copy`, `Debug`, `PartialEq`, and `Eq`.

## Construction, defaults, and validation

Factors are selected independently for RGB source, RGB destination, alpha source, and alpha destination through [`ColorBlendConfig`](color_blend_config.md). There is no factor default independent of that config's preset.

| Factor | RGB components | Alpha component |
|---|---|---|
| `Zero` | zero | zero |
| `One` | one | one |
| `SourceColor` | source RGB | source alpha |
| `OneMinusSourceColor` | one minus source RGB | one minus source alpha |
| `DestinationColor` | destination RGB | destination alpha |
| `OneMinusDestinationColor` | one minus destination RGB | one minus destination alpha |
| `SourceAlpha` | source alpha | source alpha |
| `OneMinusSourceAlpha` | one minus source alpha | one minus source alpha |
| `DestinationAlpha` | destination alpha | destination alpha |
| `OneMinusDestinationAlpha` | one minus destination alpha | one minus destination alpha |
| `ConstantColor` | constant RGB | constant alpha |
| `OneMinusConstantColor` | one minus constant RGB | one minus constant alpha |
| `ConstantAlpha` | constant alpha | constant alpha |
| `OneMinusConstantAlpha` | one minus constant alpha | one minus constant alpha |
| `SourceAlphaSaturate` | `min(source alpha, 1 - destination alpha)` | one |

The constant is [`ColorBlendConfig::blend_constants`](color_blend_config.md), in RGBA order. Vulkan source-1 factors are omitted because VMNL does not expose the required `dualSrcBlend` device feature.

## Units, coordinates, and valid ranges

Factors operate on the fragment shader's source value and the current color-attachment destination value. The selected [`ColorBlendOperation`](color_blend_operation.md) combines the weighted components. `Min` and `Max` ignore the selected factors under Vulkan's blend rules.

## Ownership, lifecycle, and threading

This is a copied CPU enum. It carries no GPU ownership or thread affinity. Pipeline build captures the selected values.

## Errors, panics, and failure conditions

The factor value itself is infallible. Pipeline build can fail when the device does not support a factor. On portability-subset devices, using `ConstantAlpha` or `OneMinusConstantAlpha` as an RGB factor requires `constantAlphaColorBlendFactors`; VMNL cannot request that feature today, so Vulkano rejects the pipeline. The target format must support `VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND`.

## Allocation, transfers, synchronization, and GPU cost

No allocation, transfer, synchronization, or wait occurs when selecting or inspecting a factor.

## Platform, Vulkan, and display constraints

Requirements follow Vulkan's color-blend feature and selected attachment format. See [`ColorBlendConfig`](color_blend_config.md) for build-time behavior.

## Example and related types

See [`ColorBlendConfig`](color_blend_config.md) for a complete configurable example.

Related: [`ColorBlendOperation`](color_blend_operation.md) and [`PipelineSpec`](../raw/pipeline/pipeline_spec.md).
