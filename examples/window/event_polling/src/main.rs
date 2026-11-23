// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Select event delivery independently from the input snapshot.

use vmnl_graphics::{Context, EventKind, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL window_event_polling")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.unconfigure_window_polling();
    window.enable_keyboard_polling();
    window.enable_window_state_polling();
    window.set_close_polling(true);
    window.set_drag_and_drop_polling(true);
    window.set_content_scale_polling(true);
    window.set_refresh_polling(true);
    println!(
        "Press Space to toggle key events. Press or release A to compare delivery and tracking."
    );
    println!("Drop a file or change display scale to observe the enabled window events.");

    while window.is_open() {
        for event in window.poll_events() {
            println!("{:?}", event.kind());
            if matches!(event.kind(), EventKind::Closed) {
                window.close();
            }
        }
        let keyboard = window.input().keyboard();
        let toggle = keyboard.is_pressed(Key::Space);
        if keyboard.is_pressed(Key::A) || keyboard.is_released(Key::A) {
            println!("A state down: {}", keyboard.is_down(Key::A));
        }
        if toggle {
            let enabled = !window.is_key_polling_enabled();
            window.set_key_polling(enabled);
            window.set_title(&format!(
                "VMNL window_event_polling | key delivery: {enabled}"
            ));
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
