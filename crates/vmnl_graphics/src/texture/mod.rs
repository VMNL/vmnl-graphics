// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Texture sources, GPU resources, pixel preparation, and sampler configuration.

mod gpu;
mod pixel;
mod png;
mod sampler;

pub(crate) use gpu::TextureRetirementRegistry;
pub use gpu::{
    PreparedTexture2D, Sampler, SamplerBuilder, Texture2D, Texture2DBuilder,
    TextureRetirementPolicy, TextureView2D, UploadTicket, UploadWaitOutcome,
};
pub use pixel::{
    AlphaMode, ColorEncoding, PixelLayout, PixelOrigin, PixelRect, PixelSource, PreparedPixelData,
    TextureFormat, TextureUsage, TextureUsageRequest,
};
pub use png::PngDecodeOptions;
pub use sampler::{
    AddressMode, Filter, SamplerBorderColor, SamplerConfig, SamplerCoordinateMode, SamplerLodLimit,
};
