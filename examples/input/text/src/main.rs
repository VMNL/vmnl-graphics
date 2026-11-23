// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Observe Unicode text delivery and the optional legacy text callback.

use vmnl_graphics::{Context, EventKind, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL input_text")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.set_char_polling(true);
    window.set_char_mods_polling(false);
    println!("Type text. F9 toggles text delivery and F10 toggles legacy modified text.");

    while window.is_open() {
        for event in window.poll_events() {
            match event.kind() {
                EventKind::Text(character) => {
                    println!("text: {character:?}");
                    window.set_title(&format!("VMNL input_text | {character}"));
                }
                EventKind::TextWithModifiers {
                    character,
                    modifiers,
                } => {
                    println!("legacy text: {character:?}, modifiers: {modifiers:?}");
                }
                _ => {}
            }
        }
        if window.input().keyboard().is_pressed(Key::F9) {
            let enabled = !window.is_char_polling_enabled();
            window.set_char_polling(enabled);
            println!("text delivery: {enabled}");
        }
        if window.input().keyboard().is_pressed(Key::F10) {
            let enabled = !window.is_char_mods_polling_enabled();
            window.set_char_mods_polling(enabled);
            println!("legacy modified text delivery: {enabled}");
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
