// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Observe mouse motion, buttons, and scroll through a window.

use vmnl_graphics::{Context, EventKind, MouseButton, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL input_mouse")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.enable_mouse_polling();
    window.set_scroll_polling(true);
    let mut scroll = (0.0, 0.0);
    let mut last_button = MouseButton::Left;
    println!("Move, click and scroll inside the window. Close it with the window controls.");

    while window.is_open() {
        let mut changed = false;
        for event in window.poll_events() {
            match event.kind() {
                EventKind::MouseScrolled { dx, dy } => {
                    println!("scroll dx={dx:.2} dy={dy:.2}");
                    scroll = (*dx, *dy);
                    changed = true;
                }
                EventKind::MouseButtonPressed { button, .. }
                | EventKind::MouseButtonReleased { button, .. } => {
                    println!("{:?}", event.kind());
                    last_button = *button;
                    changed = true;
                }
                EventKind::MouseMoved { .. } | EventKind::MouseEntered | EventKind::MouseLeft => {
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            let (x, y) = window.get_cursor_position();
            let mouse = window.input().mouse();
            let state = format!(
                "({x:.0}, {y:.0}) | {last_button:?} down={} pressed={} released={} | scroll ({:.2}, {:.2})",
                mouse.is_down(last_button), mouse.is_pressed(last_button),
                mouse.is_released(last_button), scroll.0, scroll.1,
            );
            println!("{state}");
            window.set_title(&format!("VMNL input_mouse | {state}"));
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
