// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare cursor modes and inspect raw mouse motion support.

use vmnl_graphics::{Context, CursorMode, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL input_cursor_modes")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    let raw_supported = context.is_raw_mouse_motion_supported();
    let modes = [
        (Key::N, CursorMode::Normal),
        (Key::V, CursorMode::Hidden),
        (Key::D, CursorMode::Disabled),
        (Key::G, CursorMode::Captured),
    ];
    println!("N/V/D/G selects Normal/Hidden/Disabled/Captured. P centers the cursor. In Disabled mode, M toggles raw motion when supported.");
    println!("Raw mouse motion supported: {raw_supported}");

    while window.is_open() {
        for _ in window.poll_events() {}
        let keyboard = window.input().keyboard();
        let mode = modes
            .iter()
            .find(|(key, _)| keyboard.is_pressed(*key))
            .map(|(_, mode)| *mode);
        let center = keyboard.is_pressed(Key::P);
        let motion = keyboard.is_pressed(Key::M);
        if let Some(mode) = mode {
            window.set_cursor_mode(mode)?;
        }
        if center {
            let (width, height) = window.get_size();
            window.set_cursor_position(f64::from(width) / 2.0, f64::from(height) / 2.0)?;
        }
        if motion && raw_supported && window.get_cursor_mode() == CursorMode::Disabled {
            window.set_raw_mouse_motion(!window.is_raw_mouse_motion_enabled())?;
        }
        if mode.is_some() || motion || center {
            window.set_title(&format!(
                "VMNL input_cursor_modes | {:?} | raw motion: {}",
                window.get_cursor_mode(),
                window.is_raw_mouse_motion_enabled(),
            ));
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
