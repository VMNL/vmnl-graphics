// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Texture sources, GPU resources, pixel preparation, and sampler configuration.

mod gpu;
mod pixel;
mod png;
mod sampler;

pub(crate) use gpu::{
    validate_sampled_image_sampler, SampledImageBinding, SampledSamplerBinding,
    SampledTextureBinding, TextureRetirementRegistry, TextureUploadDependency,
};
pub use gpu::{
    PreparedTexture2D, Sampler, SamplerBuilder, Texture2D, Texture2DBuilder, TextureComponent,
    TextureComponentMapping, TextureRetirementPolicy, TextureView2D, TextureView2DBuilder,
    TextureViewInterpretation, UploadTicket, UploadWaitOutcome,
};
pub use pixel::{
    AlphaMode, ColorEncoding, PixelLayout, PixelOrigin, PixelRect, PixelSource, PreparedPixelData,
    TextureFormat, TextureUsage, TextureUsageRequest,
};
pub use png::PngDecodeOptions;
pub use sampler::{
    AddressMode, Filter, SamplerBorderColor, SamplerConfig, SamplerCoordinateMode, SamplerLodLimit,
};
