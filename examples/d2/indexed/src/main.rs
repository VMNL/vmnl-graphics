// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Build a rectangle from explicitly indexed triangles.

use vmnl_graphics::{
    common::Rgba,
    d2::{Shape, Vector2f, Vertex2D},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_indexed")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let vertices = [
        Vertex2D {
            position: Vector2f { x: 190.0, y: 135.0 },
            color: Rgba::RED,
        },
        Vertex2D {
            position: Vector2f { x: 610.0, y: 135.0 },
            color: Rgba::GREEN,
        },
        Vertex2D {
            position: Vector2f { x: 610.0, y: 405.0 },
            color: Rgba::BLUE,
        },
        Vertex2D {
            position: Vector2f { x: 190.0, y: 405.0 },
            color: Rgba::WHITE,
        },
    ];
    let quad = Shape::indexed(vertices, [0, 1, 2, 0, 2, 3]).build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&quad]).submit()?;
        }
    }

    Ok(())
}
