// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Inspect the window timer and reset elapsed time.

use vmnl_graphics::{Context, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL window_timer")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.set_time(0.0);
    println!(
        "Timer frequency: {} ticks per second.",
        window.get_timer_frequency()
    );
    println!("Press Space to print elapsed time and reset it.");

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Space) {
            let seconds = window.get_time();
            println!(
                "elapsed: {seconds:.3} seconds; ticks: {}",
                window.get_timer_value()
            );
            window.set_time(0.0);
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
