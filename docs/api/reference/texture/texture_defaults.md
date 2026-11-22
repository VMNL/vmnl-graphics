# `TextureDefaults`

## Public path and maturity

Import path: `vmnl_graphics::texture::TextureDefaults`.

## Purpose and use cases

Configure exact context defaults for mutable-format image creation and the format,
component mapping, and sampled interpretation of explicit texture views.

## Public API

`Default` leaves every field unset. The setters are `with_mutable_format_views`,
`with_view_format`, `with_view_component_mapping`, and `with_view_interpretation`.
Each accepts an `Option`, where `None` clears that setting. The corresponding
getters return the configured values.

## Construction, defaults, and validation

Pass the value to `ContextBuilder::texture_defaults`. A builder resolves each
unset field from its context. If the context leaves a field unset, images use fixed
format, explicit views use the image's format and an identity mapping, and only
identity views in the storage format inherit the parent texture's interpretation.
`Texture2D::view()` always returns the texture's default view. Use
`TextureView2D::builder` to apply explicit view defaults.

## Units, coordinates, and valid ranges

Format, component mapping, and interpretation are discrete exact settings.
They do not change the texture's storage bytes.

## Ownership, lifecycle, and threading

The configuration is `Copy` and owns no backend objects. `Context` stores a
snapshot at build. Textures and views retain the policy resolved when built.

## Errors, panics, and failure conditions

Configuration is infallible. Unsupported combinations fail when preparing the
image or creating the view. A view cannot reinterpret an image created with
fixed format. See the [GPU resource contracts](gpu_resources.md) for error types.

## Allocation, transfers, synchronization, and GPU cost

Defaults are CPU values. Configuring them creates no image or sampler and
submits no GPU work.

## Platform, Vulkan, and display constraints

The inherited policy must satisfy the same Vulkan and device constraints as
the corresponding local setting. VMNL does not substitute a supported setting.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::{texture::TextureDefaults, Context};

let defaults = TextureDefaults::default().with_mutable_format_views(Some(true));
let builder = Context::builder().texture_defaults(defaults);
assert_eq!(builder.texture_defaults_config().mutable_format_views(), Some(true));
```

See [`ContextBuilder`](../context_builder.md) and the [GPU resource contracts](gpu_resources.md).
