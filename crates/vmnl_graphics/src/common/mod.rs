// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Shared graphics primitives.

mod buffer;
mod color;
mod color_blend;
mod geometry;
mod pipeline;
mod shader;

pub use buffer::BufferMemoryPreference;
pub use color::Rgba;
pub use color_blend::{
    ColorBlendConfig, ColorBlendFactor, ColorBlendOperation, ColorBlendOverrides,
};
pub use shader::ShaderSource;

pub(crate) use buffer::{GraphicsResourceFactory, IndexBuffer, VertexBuffer};
pub(crate) use geometry::{checked_draw_counts, validate_indexed_triangle_geometry, GpuGeometry};
pub(crate) use pipeline::{BlendMode, MaterialKey, PipelineKey};
