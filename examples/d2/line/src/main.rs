// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare butt, round and square caps with the same line width.

use vmnl_graphics::{
    common::Rgba,
    d2::{LineCap, Shape, Vector2f},
    Context, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL d2_line")
        .size(800, 540)
        .set_clear_color(Rgba::rgb(12, 16, 24))
        .build(&context)?;

    let lines = [LineCap::Butt, LineCap::Round, LineCap::Square]
        .into_iter()
        .zip([140.0, 260.0, 380.0])
        .map(|(cap, y)| {
            Shape::line(Vector2f { x: 175.0, y }, Vector2f { x: 625.0, y })
                .width(36.0)
                .cap(cap)
                .color(Rgba::CYAN)
                .build(&context)
        })
        .collect::<VMNLResult<Vec<_>>>()?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw2d([&lines[0], &lines[1], &lines[2]])
                .submit()?;
        }
    }

    Ok(())
}
