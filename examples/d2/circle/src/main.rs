// SPDX-FileCopyrightText: 2026 Anexoms
// SPDX-License-Identifier: MIT

//! Compare both orders of the circle fill and width setters.

use vmnl_graphics::{common::Rgba, d2::Shape, Context, Key, PresentMode, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_circle")
        .size(640, 480)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .present_mode(PresentMode::Auto)
        .build(&context)?;

    let filled_before_width = Shape::circle(24.0)
        .filled(true)
        .width(8.0)
        .position(240.0, 150.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let filled_after_width = Shape::circle(24.0)
        .width(8.0)
        .filled(true)
        .position(400.0, 150.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let outline_before_width = Shape::circle(24.0)
        .filled(false)
        .width(8.0)
        .position(240.0, 300.0)
        .color(Rgba::YELLOW)
        .build(&context)?;
    let outline_after_width = Shape::circle(24.0)
        .width(8.0)
        .filled(false)
        .position(400.0, 300.0)
        .color(Rgba::YELLOW)
        .build(&context)?;

    println!("Press Escape to close.");
    println!("Compare matching filled circles above matching outlined circles.");
    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Escape) {
            window.close();
        }
        window
            .render()
            .draw2d([
                &filled_before_width,
                &filled_after_width,
                &outline_before_width,
                &outline_after_width,
            ])
            .submit()?;
    }

    Ok(())
}
