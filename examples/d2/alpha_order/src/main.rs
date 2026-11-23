// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

//! Reverse the draw order of two overlapping translucent shapes.

use vmnl_graphics::{common::Rgba, d2::Shape, Context, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_alpha_order | blue over red (Space: reverse)")
        .size(640, 480)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let red = Shape::rect(280.0, 210.0)
        .position(100.0, 90.0)
        .color(Rgba::rgba(255, 40, 40, 128))
        .build(&context)?;
    let blue = Shape::rect(280.0, 210.0)
        .position(260.0, 180.0)
        .color(Rgba::rgba(40, 100, 255, 128))
        .build(&context)?;

    let mut red_on_top = false;
    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Space) {
            red_on_top = !red_on_top;
            let order = if red_on_top {
                "red over blue"
            } else {
                "blue over red"
            };
            window.set_title(&format!("VMNL d2_alpha_order | {order} (Space: reverse)"));
        }
        let (back, front) = if red_on_top {
            (&blue, &red)
        } else {
            (&red, &blue)
        };
        if window.is_open() {
            window.render().draw2d([back]).draw2d([front]).submit()?;
        }
    }
    Ok(())
}
