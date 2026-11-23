// SPDX-FileCopyrightText: 2026 Anexoms
// SPDX-License-Identifier: MIT

//! Draw a filled circular sector.

use vmnl_graphics::{
    common::Rgba,
    d2::{LineCap, Shape},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_sector")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let sector = Shape::circle(140.0)
        .sector(0.0, 90.0)
        .cap(LineCap::Butt)
        .filled(true)
        .segments(64)
        .position(400.0, 270.0)
        .color(Rgba::GREEN)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&sector]).submit()?;
        }
    }

    Ok(())
}
