# Surface color selection

## Public path and maturity

Import `SurfaceColorConfig`, `SurfaceColorFallback`, `SurfaceColorInfo`, `SurfaceColorPair`,
`SurfaceColorSelection`, and `SurfaceColorSpace` from `vmnl_graphics`. Target milestone: 0.2.0.

## Purpose and use cases

Choose the swapchain attachment format and the presentation engine's color space as a single
pair. Use exact selection to require a specific target for a shader or prefer compatible pairs in
application order. Texture encoding and blending are separate contracts.

## Public API

| Type | Public members |
|---|---|
| `SurfaceColorConfig` | `Default`, `legacy_auto`, `exact`, `prefer`, `prefer_with_fallback`, `is_legacy_auto`, `exact_pair`, `preferences`, `fallback` |
| `SurfaceColorPair` | `new`, `format`, `color_space` |
| `SurfaceColorSpace` | `SRGB_NON_LINEAR`, exact equality and `Display` |
| `SurfaceColorFallback` | `Reject` (default), `FirstReported` |
| `SurfaceColorInfo` | `requested`, `pair`, `selection`, `used_fallback` |
| `SurfaceColorSelection` | `Exact`, `Preference { index }`, `LegacyPreferred`, `FirstReported` |

`WindowBuilder::surface_color` sets the policy and `surface_color_value` borrows it.
`Window::surface_color_info` borrows the resolved snapshot. `Window::surface_color_pairs`
queries the current inventory of the existing surface on its context's physical device.

## Construction, defaults, and validation

The policy belongs to the window. The default prefers `B8G8R8A8_SRGB / SRGB_NON_LINEAR`, then
the first representable pair in backend order. `exact` requires both requested components in the
same reported pair. `prefer` checks pairs in caller order, rejects an empty list or duplicates,
and uses `Reject`. `prefer_with_fallback` accepts an explicit alternative. Fallback occurs only
when selecting a pair. A selected format must also support the window's alpha-blended 2D pipeline.

## Units, coordinates, and valid ranges

Pairs compare by exact format and color-space identity. Preference indices are zero-based in
caller order. Inspection preserves distinct spaces understood by the pinned Vulkan backend, even
when VMNL has no named constant for them.

## Ownership, lifecycle, and threading

The window stores the original policy and resolved pair. Normal resize keeps the pair and existing
render pass and pipelines. Inventory queries use the current window thread. The types are CPU
values, and a cloned snapshot can outlive the window. Attachment information and raw pipeline
target identity still follow [`RenderTargetInfo`](render_target_info.md).

## Errors, panics, and failure conditions

Invalid preferences return `InvalidState` before surface creation. An unmatched request returns
`SurfaceColorUnavailable` before swapchain creation. `VulkanFormatNotSupported` means the selected
format cannot support the window's alpha-blended 2D pipeline. Inventory queries may return
`VulkanSurfaceColorQueryFailed`, `VulkanSurfaceLost` or `VulkanOutOfMemory`.

During recreation, `SurfaceColorPairLost` keeps the selection fixed. Retry when the pair returns
or build a new window with a different policy. A framebuffer creation error can be retried with
the replacement swapchain. If native creation fails after retiring the old swapchain, subsequent
submissions return `SwapchainRecreationRequiresNewWindow`.

## Allocation, transfers, synchronization, and GPU cost

Preferences own an allocation. Snapshot getters do not allocate, query Vulkan or synchronize.
An inventory query performs a fresh native enumeration and returns an owned vector. Normal
recreation rechecks the pair and rebuilds its swapchain images and framebuffers. Color selection
adds no per-draw conversion, transfer or attachment.

## Platform, Vulkan, and display constraints

Reported pairs depend on the actual surface, physical device and active Vulkan extensions.
VMNL excludes color spaces that require an inactive extension from the inventory and selection.
Revalidate copied pairs when creating another window. The format controls attachment encoding,
including conversion of clear and fragment output. The presentation color space controls the
interpretation of those pixels by the presentation engine. Window alpha composition belongs to
the platform's presentation path, independent of blend equations inside the attachment.

## Example and related types

Run [`window_surface_color`](../../../../examples/window/surface_color/src/main.rs) to inspect
the selected pair and resize the window. See [`WindowBuilder`](window_builder.md),
[`Window`](window.md) and [`ColorFormat`](color_format.md).
