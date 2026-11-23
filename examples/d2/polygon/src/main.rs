// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare convex and concave polygons with vertex colors.

use vmnl_graphics::{
    common::Rgba,
    d2::{Shape, Vector2f},
    Context, VMNLResult, Window,
};

fn v2(x: f32, y: f32) -> Vector2f {
    Vector2f { x, y }
}

fn polygon_pentagon(context: &Context) -> VMNLResult<Shape> {
    Shape::polygon([
        v2(220.0, 180.0),
        v2(390.0, 305.0),
        v2(325.0, 505.0),
        v2(115.0, 505.0),
        v2(50.0, 305.0),
    ])
    .vertex_colors([Rgba::RED, Rgba::YELLOW, Rgba::GREEN, Rgba::CYAN, Rgba::BLUE])
    .build(context)
}

fn concave_polygon(context: &Context) -> VMNLResult<Shape> {
    Shape::polygon([
        v2(525.0, 180.0),
        v2(785.0, 180.0),
        v2(640.0, 305.0),
        v2(785.0, 450.0),
        v2(525.0, 450.0),
    ])
    .vertex_colors([
        Rgba::PURPLE,
        Rgba::BLUE_VIOLET,
        Rgba::BLUE,
        Rgba::CYAN,
        Rgba::GREEN,
    ])
    .build(context)
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_polygon")
        .size(920, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let convex = polygon_pentagon(&context)?;
    let concave = concave_polygon(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&convex, &concave]).submit()?;
        }
    }

    Ok(())
}
