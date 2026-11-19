# Texture API

This namespace contains borrowed pixel descriptors, static PNG decoding,
GPU-backed image/view/sampler resources, and explicit initial-upload control.
Raw pipelines can sample a `Texture2D` default view through a combined
image-sampler descriptor. High-level 2D sampling and configurable image views
remain later texture steps.

- [Pixel descriptors and preparation](descriptors.md)
- [Static PNG decoding](png_decode.md)
- [Sampler configuration](sampler_config.md)
- [GPU resources and upload lifecycle](gpu_resources.md)
