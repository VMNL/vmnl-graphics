// SPDX-FileCopyrightText: 2026 Anexoms
// SPDX-License-Identifier: MIT

//! Draw rings with an explicit width or inner offset.

use vmnl_graphics::{common::Rgba, d2::Shape, Context, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_ring")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let with_width = Shape::circle(92.0)
        .width(24.0)
        .position(240.0, 270.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let with_inner_offset = Shape::circle(92.0)
        .inner(48.0)
        .position(560.0, 270.0)
        .color(Rgba::YELLOW)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw2d([&with_width, &with_inner_offset])
                .submit()?;
        }
    }

    Ok(())
}
