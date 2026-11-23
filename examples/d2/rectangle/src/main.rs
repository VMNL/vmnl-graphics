// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Draw a solid rectangle.

use vmnl_graphics::{common::Rgba, d2::Shape, Context, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_rectangle")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let rectangle = Shape::rect(280.0, 180.0)
        .position(260.0, 180.0)
        .color(Rgba::CYAN)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&rectangle]).submit()?;
        }
    }

    Ok(())
}
