// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! 2D graphics resources and public types.

mod render_item;
mod shape;
mod vector;
mod vertex;

pub use render_item::{Drawable2D, RenderItem2D};
pub use shape::{
    Anchor, CornerRadii, IndexedShapeBuilder, LineBuilder, LineCap, LineJoin, PolygonBuilder,
    PolylineBuilder, RectBuilder, Shape, TexturedShape, TexturedShapeBuilder, TexturedVertex2D,
    TriangleBuilder,
};
pub use vector::Vector2f;
pub use vertex::Vertex2D;

pub(crate) use render_item::Draw2D;
pub(crate) use shape::{GpuTexturedVertex2D, TexturedDraw};
pub(crate) use vertex::GpuVertex2D;
