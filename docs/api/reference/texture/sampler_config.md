# Sampler configuration

## Public path and maturity

Import sampler request values and GPU handles from `vmnl_graphics::texture`.
`SamplerConfig` is a CPU-owned request; `SamplerBuilder` checks the request
against the selected device before creating the Vulkan sampler.

## Purpose and use cases

Configure filtering, address modes, border color, LOD range and bias,
anisotropy, and coordinate interpretation before creating a sampler.

## Public API

### Filter

`Nearest` selects the closest sample; `Linear` interpolates samples.

### AddressMode

`ClampToEdge`, `Repeat`, `MirroredRepeat`, and `ClampToBorder` select the
addressing behavior for one texture axis.

### SamplerBorderColor

`FloatTransparentBlack`, `IntTransparentBlack`, `FloatOpaqueBlack`,
`IntOpaqueBlack`, `FloatOpaqueWhite`, and `IntOpaqueWhite` retain the Vulkan
numeric class in the request. [`TextureView2D::validate_sampler`](gpu_resources.md#textureview2d)
checks the border class against a specific view. Shader compatibility remains
the responsibility of a sampling consumer.

### SamplerLodLimit

`Unbounded` represents a client-unbounded maximum without a magic float;
`Value(f32)` requests an explicit finite maximum.

### SamplerCoordinateMode

`Normalized` uses normalized sampling coordinates. `Texel` requests Vulkan
unnormalized coordinates and the stricter compatibility rules described below.

### SamplerConfig

`default()` and `linear()` request linear minification, magnification, and
mipmap filtering; U/V clamp-to-edge; floating transparent-black border; minimum
LOD `0`; unbounded maximum LOD; bias `0`; anisotropy `1` (disabled); and
normalized coordinates. `nearest()` changes all three filters to nearest and
preserves every other default.

The `with_*` methods return edited values. Getters expose each requested
setting. `validate` rejects non-finite LOD/anisotropy values, maximum LOD below
minimum LOD, anisotropy below `1`, and incompatible texel-mode settings.
`SamplerBuilder::build` rejects anisotropy greater than `1` unless
`DeviceFeature::SamplerAnisotropy` was enabled, rejects values above the device
limit, and checks nonzero LOD bias against its limit. A nonzero LOD bias on
portability-subset devices also requires `DeviceFeature::SamplerMipLodBias`.
Requests are not clamped. Border/view numeric compatibility and shader
instruction compatibility require the later sampling consumer.

Texel coordinates require equal min/mag filters, nearest mip filtering, zero
minimum/maximum LOD and bias, anisotropy `1`, and ClampToEdge or ClampToBorder on
both axes. The associated image/view and shader restrictions are checked at the
consumer boundary; independent requests are never silently rewritten.

## Construction, defaults, and validation

All settings are explicit values. Presets are editable and independent; changing
one field preserves the others. `SamplerConfig` does not query a device or
create Vulkan objects.

## Units, coordinates, and valid ranges

LOD values, LOD bias, and anisotropy are finite `f32` values. Anisotropy is at
least `1`. `SamplerLodLimit::Unbounded` is distinct from any finite value.
Normalized and texel coordinates use Vulkan sampler semantics.

## Ownership, lifecycle, and threading

Configuration values are copied CPU data and are not bound to a context or
thread.

## Errors, panics, and failure conditions

Numerically invalid requests, texel-mode conflicts, and sampler values outside
the device limits return `VMNLErrorKind::InvalidState`. A required optional
feature that was not enabled returns `DeviceFeatureNotEnabled`. View/shader
incompatibilities are checked by the later sampling consumer.

## Allocation, transfers, synchronization, and GPU cost

Configuration and validation allocate no heap memory and issue no GPU work.
`SamplerBuilder::build` allocates one Vulkan sampler; no sampler cache is used.

## Platform, Vulkan, and display constraints

Values model Vulkan sampler settings. Device support and platform-specific
sampler limits are checked when a context-bound sampler is created.

## Example and related types

```rust
# extern crate vmnl_graphics;
# fn main() -> vmnl_graphics::VMNLResult<()> {
use vmnl_graphics::texture::{AddressMode, Filter, SamplerConfig};

let config = SamplerConfig::linear()
    .with_min_filter(Filter::Nearest)
    .with_address_u(AddressMode::Repeat);
assert_eq!(config.min_filter(), Filter::Nearest);
assert_eq!(config.mag_filter(), Filter::Linear);
config.validate()?;
# Ok(())
# }
```

Related: [pixel descriptors](descriptors.md) and [PNG decoding](png_decode.md).
