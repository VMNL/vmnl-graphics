// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare rectangle rotation about a center anchor and a custom origin.

use vmnl_graphics::{
    common::Rgba,
    d2::{Anchor, Shape},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_rectangle_transform")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let centered = Shape::rect(180.0, 100.0)
        .position(225.0, 250.0)
        .color(Rgba::rgba(255, 90, 90, 220))
        .anchor(Anchor::Center)
        .rotation(30.0)
        .build(&context)?;
    let custom_origin = Shape::rect(160.0, 90.0)
        .position(540.0, 260.0)
        .color(Rgba::rgba(90, 180, 255, 220))
        .origin(20.0, 70.0)
        .rotation(-25.0)
        .build(&context)?;
    let center_marker = Shape::rect(8.0, 8.0)
        .anchor(Anchor::Center)
        .position(225.0, 250.0)
        .color(Rgba::WHITE)
        .build(&context)?;
    let origin_marker = Shape::rect(8.0, 8.0)
        .anchor(Anchor::Center)
        .position(540.0, 260.0)
        .color(Rgba::WHITE)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw2d([&centered, &custom_origin, &center_marker, &origin_marker])
                .submit()?;
        }
    }

    Ok(())
}
