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
| Owned elapsed-time clocks (`time::Clock`, `time::Duration`) | Accepted for 0.2.0, not in the 0.1.0 baseline | Headless unit/API/doctest evidence; no GPU, window, or display required |
| Raw typed pipeline/geometry | Operational in 0.1.0 | Restricted public contract |
| Raw configurable color blending | Partial 0.2.0 | Legacy exhaustive `BlendMode` preserved; editable RGB/alpha factors and operations, three presets, constants and effective pipeline inspection; dual-source factors unavailable |
| Raw uniform buffers/resources | Operational in 0.1.0 | Scalar uniform-buffer descriptors |
| Raw texture sampling | Partial 0.2.0 | Scalar combined or separate image/sampler descriptors; default `Texture2D` view only; shader/sampler compatibility is checked |
| Raw depth-comparison sampling and samplerless `texelFetch` | Unsupported | Outside the raw texture-sampling contract |
| Render-target inspection / preflight | Operational in 0.1.0 | Actual color/depth formats and samples; strict originating-window identity; resize preserves validity |
| Optional window depth attachments / raw depth tests | Operational in 0.1.0 | Explicit D16/D32, per-image ownership and resize; compare/write state on raw pipelines |
| Texture pixel descriptors, static PNG decode, Texture2D upload, default view, sampler, readback, deferred retirement, and raw default-view sampling | Partial `0.2.0` resource foundation | CPU/API and GPU/display checks cover upload, sampler creation, exact readback, raw combined and separate sampling, context identity, ticket error persistence, and both retirement selections; configurable views, source retention, resource capability/memory/tiling controls, and high-level 2D sampling are not implemented; an actual in-flight GPU `Wait` and Vulkan upload-failure transition are not exercised |
| Raw explicit texture views, storage, descriptor arrays, and push constants | Unsupported | Rejected by pipeline/resource validation |
| `RenderMode::Batched` optimization | Not implemented | Falls back to `PerObject` |

Compilation, doctests, and headless tests do not prove Vulkan execution or correct pixels. No performance/synchronization claim beyond documented behavior is specified.
