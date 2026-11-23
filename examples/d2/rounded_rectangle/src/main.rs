// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare uniform and per-corner rectangle radii.

use vmnl_graphics::{
    common::Rgba,
    d2::{CornerRadii, Shape},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_rounded_rectangle")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let rounded_uniform = Shape::rect(200.0, 100.0)
        .position(100.0, 90.0)
        .corner_radius(24.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let rounded_asymmetric = Shape::rect(220.0, 90.0)
        .position(470.0, 90.0)
        .corner_radii(CornerRadii::new(28.0, 6.0, 40.0, 0.0))
        .color(Rgba::MAGENTA)
        .build(&context)?;
    let rounded_rotated = Shape::rect(150.0, 80.0)
        .position(200.0, 330.0)
        .corner_radius(16.0)
        .rotation(20.0)
        .color(Rgba::YELLOW)
        .build(&context)?;
    let rounded_partially_sharp = Shape::rect(170.0, 80.0)
        .position(490.0, 290.0)
        .corner_radii(CornerRadii::new(24.0, 24.0, 0.0, 0.0))
        .color(Rgba::GREEN)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw2d([
                    &rounded_uniform,
                    &rounded_asymmetric,
                    &rounded_rotated,
                    &rounded_partially_sharp,
                ])
                .submit()?;
        }
    }

    Ok(())
}
