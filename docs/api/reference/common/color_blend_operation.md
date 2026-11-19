# `ColorBlendOperation`

## Public path and maturity

Import path: `vmnl_graphics::common::ColorBlendOperation`.

## Purpose and use cases

Chooses how the weighted source and destination components are combined for RGB and alpha independently.

## Public API

Variants: `Add`, `Subtract`, `ReverseSubtract`, `Min`, and `Max`. The enum is `#[non_exhaustive]` and derives `Clone`, `Copy`, `Debug`, `PartialEq`, and `Eq`.

## Construction, defaults, and validation

Select operations through [`ColorBlendConfig::with_color_blend`](color_blend_config.md) and [`ColorBlendConfig::with_alpha_blend`](color_blend_config.md). The presets use `Add` for both RGB and alpha.

| Operation | Result before attachment format conversion |
|---|---|
| `Add` | weighted source + weighted destination |
| `Subtract` | weighted source - weighted destination |
| `ReverseSubtract` | weighted destination - weighted source |
| `Min` | minimum of source and destination |
| `Max` | maximum of source and destination |

`Min` and `Max` ignore blend factors, as specified by Vulkan.

## Units, coordinates, and valid ranges

The operation is applied separately to each RGB component and to alpha, using the factors in [`ColorBlendConfig`](color_blend_config.md). Integer and normalized attachment formats can have format-specific blending support and conversion behavior.

## Ownership, lifecycle, and threading

This is a copied CPU enum with no GPU ownership or thread affinity. Pipeline build captures the selected operation.

## Errors, panics, and failure conditions

Selection is infallible. Pipeline build can fail if the selected attachment format or device does not support the requested blend state.

## Allocation, transfers, synchronization, and GPU cost

No allocation, transfer, synchronization, or wait occurs when selecting or inspecting an operation.

## Platform, Vulkan, and display constraints

The selected color attachment format must support Vulkan's `VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND` when blending is enabled. See [`ColorBlendConfig`](color_blend_config.md) for the format requirement and pipeline error behavior.

## Example and related types

See [`ColorBlendConfig`](color_blend_config.md) for a complete configurable example.

Related: [`ColorBlendFactor`](color_blend_factor.md), [`PipelineSpec`](../raw/pipeline/pipeline_spec.md), and [`Pipeline`](../raw/pipeline/pipeline.md).
