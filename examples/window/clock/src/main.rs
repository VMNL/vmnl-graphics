// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

//! Pause one elapsed-time clock while a second clock keeps running.

use vmnl_graphics::{time::Clock, Context, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .size(680, 320)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;

    let mut controlled = Clock::new();
    let reference = Clock::new();
    println!("Space pauses or resumes A. R restarts A. B keeps running.");

    while window.is_open() {
        for _ in window.poll_events() {}
        let keyboard = window.input().keyboard();
        if keyboard.is_pressed(Key::Space) {
            if controlled.is_paused() {
                controlled.resume();
            } else {
                controlled.pause();
            }
        }
        if keyboard.is_pressed(Key::R) {
            let previous = controlled.restart();
            println!(
                "A restarted after {:.2} active seconds.",
                previous.as_secs_f64()
            );
        }
        let status = if controlled.is_paused() {
            "paused"
        } else {
            "running"
        };
        window.set_title(&format!(
            "VMNL window_clock | A ({status}) {:.2}s | B {:.2}s | Space: pause, R: restart",
            controlled.elapsed().as_secs_f64(),
            reference.elapsed().as_secs_f64(),
        ));
        if window.is_open() {
            window.render().submit()?;
            window.wait_events_timeout(0.05);
        }
    }
    Ok(())
}
