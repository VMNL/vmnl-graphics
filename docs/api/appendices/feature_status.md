# Feature status

Capabilities of the current workspace API. Existing behavior is based on `vmnl_graphics 0.1.0`;
the texture resource foundation targets `0.2.0`. Versioning does not qualify a native platform.

| Capability | Status | Evidence boundary |
|---|---|---|
| Context/device initialization | Operational in 0.1.0 | GPU/display runtime required for execution |
| Native window, events, input, monitors | Operational in 0.1.0 | GLFW/display runtime required |
| 2D predefined/indexed shapes | Operational in 0.1.0 | Unit/API checks plus GPU/examples for runtime visuals |
| Custom 2D shaders | Operational in 0.1.0 | Shader compile and visual example |
| Ordered 2D/raw frame composition | Operational in 0.1.0 | GPU/example behavior |
| Raw typed pipeline/geometry | Operational in 0.1.0 | Restricted public contract |
| Raw uniform buffers/resources | Operational in 0.1.0 | Single uniform-buffer descriptors only |
| Render-target inspection / preflight | Operational in 0.1.0 | Actual color/depth formats and samples; strict originating-window identity; resize preserves validity |
| Optional window depth attachments / raw depth tests | Operational in 0.1.0 | Explicit D16/D32, per-image ownership and resize; compare/write state on raw pipelines |
| Texture pixel descriptors, static PNG decode, Texture2D upload, default view, sampler, readback, and deferred retirement | Partial `0.2.0` resource foundation | CPU/API and GPU/display checks cover upload, sampler creation, exact readback, context identity, ticket error persistence, and both retirement selections; configurable views, source retention, resource capability/memory/tiling controls, and public renderer sampling are not implemented; an actual in-flight GPU `Wait` and Vulkan upload-failure transition are not exercised |
| Raw textures/samplers/storage/descriptors arrays/push constants | Unsupported | Rejected by pipeline/resource validation |
| `RenderMode::Batched` optimization | Not implemented | Falls back to `PerObject` |

Compilation, doctests, and headless tests do not prove Vulkan execution or correct pixels. No performance/synchronization claim beyond documented behavior is specified.
