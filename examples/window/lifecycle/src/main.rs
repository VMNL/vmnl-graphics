// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Observe focus, visibility, iconification, maximization and restoration.

use vmnl_graphics::{Context, Key, VMNLErrorKind, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL window_lifecycle")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.enable_window_state_polling();
    println!("F focus, I minimize, M maximize, R restore, H hide/show, Escape close.");
    let mut last_state = None;
    while window.is_open() {
        for _ in window.poll_events() {}
        let keyboard = window.input().keyboard();
        let pressed = |key| keyboard.is_pressed(key);
        let (focus, iconify, maximize, restore, hide, close) = (
            pressed(Key::F),
            pressed(Key::I),
            pressed(Key::M),
            pressed(Key::R),
            pressed(Key::H),
            pressed(Key::Escape),
        );
        if focus {
            window.focus();
        }
        if iconify {
            window.iconify();
        }
        if maximize {
            window.maximize();
        }
        if restore {
            window.restore();
        }
        if hide {
            window.hide();
            println!("Visible after hide: {}", window.is_visible());
            window.show();
        }
        if close {
            window.close();
        }
        let state = (
            window.is_visible(),
            window.is_focused(),
            window.is_iconified(),
            window.is_maximized(),
        );
        if last_state != Some(state) {
            let (visible, focused, minimized, maximized) = state;
            println!("ready={} visible={visible} focused={focused} minimized={minimized} maximized={maximized}", window.is_ready());
            last_state = Some(state);
        }
        if window.is_open() && !window.is_iconified() {
            match window.render().submit() {
                Ok(()) => {}
                Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => {
                    window.wait_events_timeout(0.05);
                }
                Err(error) => return Err(error),
            }
        }
    }

    Ok(())
}
