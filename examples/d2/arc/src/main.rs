// SPDX-FileCopyrightText: 2026 Anexoms
// SPDX-License-Identifier: MIT

//! Compare butt and round caps on outlined elliptical arcs.

use vmnl_graphics::{
    common::Rgba,
    d2::{LineCap, Shape},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_arc")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let butt = Shape::circle(135.0)
        .ellipse(95.0)
        .filled(false)
        .width(25.0)
        .sector(25.0, 120.0)
        .cap(LineCap::Butt)
        .position(210.0, 220.0)
        .color(Rgba::YELLOW)
        .build(&context)?;
    let round = Shape::circle(135.0)
        .ellipse(95.0)
        .filled(false)
        .width(25.0)
        .sector(25.0, 120.0)
        .cap(LineCap::Round)
        .position(570.0, 220.0)
        .color(Rgba::YELLOW)
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window.render().draw2d([&butt, &round]).submit()?;
        }
    }

    Ok(())
}
