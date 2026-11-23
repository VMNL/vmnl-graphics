// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Draw triangles with a color at each vertex.

use vmnl_graphics::{
    common::Rgba,
    d2::{Shape, Vector2f, Vertex2D},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_triangle")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let from_points = Shape::triangle(
        Vector2f { x: 95.0, y: 115.0 },
        Vector2f { x: 310.0, y: 160.0 },
        Vector2f { x: 135.0, y: 405.0 },
    )
    .vertex_colors(Rgba::YELLOW, Rgba::MAGENTA, Rgba::CYAN)
    .build(&context)?;
    let from_vertices = Shape::triangle_from_vertices([
        Vertex2D {
            position: Vector2f { x: 495.0, y: 115.0 },
            color: Rgba::YELLOW,
        },
        Vertex2D {
            position: Vector2f { x: 710.0, y: 160.0 },
            color: Rgba::MAGENTA,
        },
        Vertex2D {
            position: Vector2f { x: 535.0, y: 405.0 },
            color: Rgba::CYAN,
        },
    ])
    .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw2d([&from_points, &from_vertices])
                .submit()?;
        }
    }

    Ok(())
}
