// SPDX-FileCopyrightText: 2026 Anexoms
// SPDX-License-Identifier: MIT

//! Minimal 2D shape rendering workflow.

use vmnl_graphics::{
    common::Rgba,
    d2::{LineCap, Shape},
    Context, Key, PresentMode, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL 2D shapes")
        .size(900, 600)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .present_mode(PresentMode::Auto)
        .build(&context)?;

    let ellipse = Shape::circle(61.0)
        .ellipse(42.0)
        .width(12.0)
        .position(140.0, 125.0)
        .color(Rgba::rgba(0, 0, 255, 255))
        .build(&context)?;
    let sector = Shape::circle(70.0)
        .sector(0.0, 90.0)
        .cap(LineCap::Butt)
        .position(140.0, 325.0)
        .color(Rgba::rgba(0, 255, 0, 255))
        .segments(32)
        .build(&context)?;
    let ring = Shape::circle(40.0)
        .filled(false)
        .inner(32.0)
        .position(340.0, 325.0)
        .color(Rgba::rgba(255, 0, 0, 255))
        .segments(32)
        .build(&context)?;
    let circle = Shape::circle(65.0)
        .position(340.0, 125.0)
        .color(Rgba::rgba(255, 0, 255, 255))
        .build(&context)?;

    let arc = Shape::circle(200.0)
        .filled(false)
        .width(25.0)
        .sector(25.0, 120.0)
        .ellipse(150.0)
        .cap(LineCap::Round)
        .position(450.0, 250.0)
        .color(Rgba::rgba(225, 123, 16, 255))
        .build(&context)?;

    let filled_before_width = Shape::circle(24.0)
        .filled(true)
        .width(8.0)
        .position(690.0, 110.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let filled_after_width = Shape::circle(24.0)
        .width(8.0)
        .filled(true)
        .position(780.0, 110.0)
        .color(Rgba::CYAN)
        .build(&context)?;
    let outline_before_width = Shape::circle(24.0)
        .filled(false)
        .width(8.0)
        .position(690.0, 200.0)
        .color(Rgba::YELLOW)
        .build(&context)?;
    let outline_after_width = Shape::circle(24.0)
        .width(8.0)
        .filled(false)
        .position(780.0, 200.0)
        .color(Rgba::YELLOW)
        .build(&context)?;

    println!("Press Escape to close.");
    println!("Right: matching filled circles above matching outlined circles.");
    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Escape) {
            window.close();
        }
        window
            .render()
            .draw2d([
                &circle,
                &ellipse,
                &arc,
                &ring,
                &sector,
                &filled_before_width,
                &filled_after_width,
                &outline_before_width,
                &outline_after_width,
            ])
            .submit()?;
    }

    Ok(())
}
