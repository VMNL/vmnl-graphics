// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Draw joined segments and compare polyline joins, caps and color modes.

use vmnl_graphics::{
    common::Rgba,
    d2::{LineCap, LineJoin, Shape, Vector2f},
    Context, VMNLResult, Window,
};

fn v2(x: f32, y: f32) -> Vector2f {
    Vector2f { x, y }
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_polyline")
        .size(960, 480)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let polyline_bevel = Shape::polyline([
        v2(40.0, 210.0),
        v2(110.0, 300.0),
        v2(180.0, 210.0),
        v2(260.0, 310.0),
    ])
    .width(18.0)
    .cap(LineCap::Butt)
    .join(LineJoin::Bevel)
    .segment_colors([Rgba::YELLOW, Rgba::RED, Rgba::YELLOW])
    .build(&context)?;
    let polyline_miter = Shape::polyline([
        v2(350.0, 310.0),
        v2(420.0, 210.0),
        v2(490.0, 310.0),
        v2(580.0, 210.0),
    ])
    .width(18.0)
    .cap(LineCap::Round)
    .join(LineJoin::Miter)
    .miter_limit(3.0)
    .point_colors([Rgba::RED, Rgba::YELLOW, Rgba::GREEN, Rgba::BLUE])
    .build(&context)?;
    let polyline_round = Shape::polyline([
        v2(650.0, 210.0),
        v2(730.0, 310.0),
        v2(810.0, 210.0),
        v2(890.0, 310.0),
    ])
    .width(18.0)
    .cap(LineCap::Square)
    .join(LineJoin::Round)
    .color(Rgba::MAGENTA)
    .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw2d([&polyline_bevel, &polyline_miter, &polyline_round])
                .submit()?;
        }
    }

    Ok(())
}
