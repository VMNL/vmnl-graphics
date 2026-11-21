# Texture API

This namespace contains borrowed pixel descriptors, static PNG decoding,
GPU-backed image/view/sampler resources, and explicit initial-upload control.
Raw pipelines can sample the default or explicit `TextureView2D` through
combined or separate image/sampler descriptors. High-level 2D sampling remains
a later texture step.

- [Pixel descriptors and preparation](descriptors.md)
- [Static PNG decoding](png_decode.md)
- [Sampler configuration](sampler_config.md)
- [GPU resources and upload lifecycle](gpu_resources.md)
- [TextureDefaults](texture_defaults.md)
