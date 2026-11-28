// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! 2D render item descriptors.

use super::{GpuVertex2D, TexturedDraw};
use crate::common::{BlendMode, IndexBuffer, MaterialKey, PipelineKey, VertexBuffer};
use std::sync::Arc;

/// Minimal backend draw description emitted by 2D renderable objects.
#[derive(Clone)]
pub struct RenderItem2D {
    pub(crate) draw: Draw2D,
}

#[derive(Clone)]
pub(crate) enum Draw2D {
    Color(ColorDraw),
    Textured(Arc<TexturedDraw>),
}

#[derive(Clone)]
pub(crate) struct ColorDraw {
    /// Pipeline family required to draw the item.
    pub(crate) pipeline_key: PipelineKey,
    /// Material family required to draw the item.
    pub(crate) material_key: MaterialKey,
    /// Color blending policy required to draw the item.
    pub(crate) blend_mode: BlendMode,
    /// Vertex buffer consumed by the active pipeline.
    pub(crate) vertex_buffer: VertexBuffer<GpuVertex2D>,
    /// Optional index buffer for indexed geometry.
    pub(crate) index_buffer: Option<IndexBuffer>,
    /// Number of vertices to draw when no index buffer is present.
    pub(crate) vertex_count: u32,
    /// Number of indices when an index buffer is present.
    pub(crate) index_count: u32,
}

/// Contract between high-level 2D drawables and the render backend.
pub trait Drawable2D {
    /// Returns the 2D draw item that VMNL can queue in the current frame.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, d2::{Drawable2D, Shape}};
    ///
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let shape = Shape::rect(40.0, 20.0).build(&context)?;
    /// let _draw_item = shape.render_item_2d();
    /// # Ok(())
    /// # }
    /// ```
    fn render_item_2d(&self) -> RenderItem2D;
}
