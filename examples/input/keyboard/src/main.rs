// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Observe physical keyboard events and the public keyboard snapshot.

use vmnl_graphics::{Context, Event, EventKind, Key, VMNLResult, Window};

fn print_event(context: &Context, event: &Event) {
    match event.kind() {
        EventKind::KeyPressed {
            key,
            scancode,
            modifiers,
            repeat,
        } => print_key_event(
            context,
            event.timestamp_seconds(),
            "pressed",
            *key,
            *scancode,
            modifiers,
            Some(*repeat),
        ),
        EventKind::KeyReleased {
            key,
            scancode,
            modifiers,
        } => print_key_event(
            context,
            event.timestamp_seconds(),
            "released",
            *key,
            *scancode,
            modifiers,
            None,
        ),
        _ => {}
    }
}

fn print_key_event(
    context: &Context,
    timestamp: f64,
    action: &str,
    key: Key,
    event_scancode: vmnl_graphics::Scancode,
    modifiers: &vmnl_graphics::Modifiers,
    repeat: Option<bool>,
) {
    let queried_scancode = context.get_key_scancode(key);
    println!(
        "[key @ {timestamp:.6}s] action={action} key={key:?} name={:?} \
         event_scancode={} queried_scancode={queried_scancode:?} scancode_name={:?} \
         scancode_matches={} modifiers={modifiers:?} repeat={repeat:?}",
        context.get_key_name(key),
        event_scancode.as_raw(),
        context.get_scancode_name(event_scancode),
        queried_scancode == Some(event_scancode),
    );
}

fn print_key_query(context: &Context, key: Key) {
    let scancode = context.get_key_scancode(key);
    println!(
        "key query: key={key:?} name={:?} scancode={scancode:?} scancode_name={:?}",
        context.get_key_name(key),
        scancode.and_then(|value| context.get_scancode_name(value)),
    );
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL input_keyboard")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.enable_keyboard_polling();
    window.set_lock_key_modifier_reporting(true);
    for key in [Key::A, Key::Semicolon, Key::Kp0, Key::Escape] {
        print_key_query(&context, key);
    }
    println!("Press, hold and release keys. F2 toggles sticky keys. Escape closes the window.");

    while window.is_open() {
        let mut last_key = None;
        for event in window.poll_events() {
            print_event(&context, &event);
            match event.kind() {
                EventKind::KeyPressed { key, .. } | EventKind::KeyReleased { key, .. } => {
                    last_key = Some(*key);
                }
                _ => {}
            }
        }
        let keyboard = window.input().keyboard();
        let escape = keyboard.is_pressed(Key::Escape);
        let toggle_sticky = keyboard.is_pressed(Key::F2);
        if let Some(key) = last_key {
            let state = format!(
                "{key:?}: down={} pressed={} released={}",
                keyboard.is_down(key),
                keyboard.is_pressed(key),
                keyboard.is_released(key),
            );
            println!("[keyboard snapshot] {state}");
            window.set_title(&format!("VMNL input_keyboard | {state}"));
        }
        if toggle_sticky {
            let enabled = !window.is_sticky_keys_enabled();
            window.set_sticky_keys(enabled);
            println!("sticky keys: {enabled}");
        }
        if escape {
            window.close();
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
