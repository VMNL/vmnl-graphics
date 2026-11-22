# `ColorBlendOverrides`

## Public path and maturity

Import path: `vmnl_graphics::common::ColorBlendOverrides`. This type configures raw pipelines.

## Purpose and use cases

Select individual blend fields and inherit the rest from the window's context.
The same value type configures context defaults through `ContextBuilder::raw_color_blend`.

## Public API

`Default` leaves every field unset. `from_config` takes a complete
[`ColorBlendConfig`](color_blend_config.md) and sets every field. The `with_*`
methods set the enable flag, RGB source and destination factors, RGB operation,
alpha source and destination factors, alpha operation, or RGBA constants.
Each corresponding getter returns the requested value as an `Option`.

## Construction, defaults, and validation

`PipelineSpec::color_blend_overrides` sets a partial pipeline request.
Pipeline build resolves each field from that request, then the context request,
then `ColorBlendConfig::opaque()`. A complete preset or legacy mode fixes every
field. `resolve(base)` performs the same field-wise substitution against an
explicit, complete base configuration.

## Units, coordinates, and valid ranges

Uses the same factors, operations, and RGBA constant semantics as
[`ColorBlendConfig`](color_blend_config.md).

## Ownership, lifecycle, and threading

The value is `Copy` and owns no backend state. A successfully built raw pipeline
captures its effective configuration and stays tied to its build window.

## Errors, panics, and failure conditions

Setting and resolving fields are infallible. Unsupported device or attachment
combinations fail during pipeline build with `VulkanPipelineCreationFailed`.

## Allocation, transfers, synchronization, and GPU cost

Setting, inspecting, and resolving fields are CPU operations without allocation,
GPU queries, or submission.

## Platform, Vulkan, and display constraints

Pipeline build applies the same format and device requirements as
[`ColorBlendConfig`](color_blend_config.md). It performs no fallback.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::common::{ColorBlendConfig, ColorBlendOperation, ColorBlendOverrides};
use vmnl_graphics::raw::Pipeline;
use vmnl_graphics::Context;

let defaults = ColorBlendOverrides::from_config(ColorBlendConfig::straight_alpha());
let context = Context::builder().raw_color_blend(defaults);
let spec = Pipeline::<[f32; 2]>::builder()
    .color_blend_overrides(ColorBlendOverrides::default().with_alpha_operation(ColorBlendOperation::Max));
let base = context.raw_color_blend_defaults().resolve(ColorBlendConfig::opaque());
let planned = spec.color_blend_overrides_value().resolve(base);
assert!(planned.blend_enabled());
assert_eq!(planned.alpha_operation(), ColorBlendOperation::Max);
```

See [`ContextBuilder`](../context_builder.md) and [`PipelineSpec`](../raw/pipeline/pipeline_spec.md).
