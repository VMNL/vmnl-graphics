// SPDX-FileCopyrightText: 2026 Anexoms
// SPDX-License-Identifier: MIT

//! Draw a filled and an outlined ellipse.

use vmnl_graphics::{common::Rgba, d2::Shape, Context, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_ellipse")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let filled = Shape::circle(130.0)
        .ellipse(78.0)
        .filled(true)
        .position(220.0, 270.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let outlined = Shape::circle(130.0)
        .ellipse(78.0)
        .filled(false)
        .width(12.0)
        .position(580.0, 270.0)
        .color(Rgba::YELLOW)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&filled, &outlined]).submit()?;
        }
    }

    Ok(())
}
