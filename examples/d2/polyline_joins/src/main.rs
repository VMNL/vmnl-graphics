// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

//! Compare polyline joins with identical geometry and stroke settings.

use vmnl_graphics::{
    common::Rgba,
    d2::{LineCap, LineJoin, Shape, Vector2f},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_polyline_joins | bevel (left), miter (center), round (right)")
        .size(900, 450)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let stroke = |center, join| {
        Shape::polyline([
            Vector2f {
                x: center - 70.0,
                y: 310.0,
            },
            Vector2f {
                x: center,
                y: 140.0,
            },
            Vector2f {
                x: center + 70.0,
                y: 310.0,
            },
        ])
        .width(30.0)
        .cap(LineCap::Butt)
        .color(Rgba::CYAN)
        .miter_limit(4.0)
        .join(join)
        .build(&context)
    };
    let bevel = stroke(150.0, LineJoin::Bevel)?;
    let miter = stroke(450.0, LineJoin::Miter)?;
    let round = stroke(750.0, LineJoin::Round)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&bevel, &miter, &round]).submit()?;
        }
    }
    Ok(())
}
