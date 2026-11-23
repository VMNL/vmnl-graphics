// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Configure and inspect window size, aspect ratio and placement.

use vmnl_graphics::{Context, Key, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL window_config")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.set_error_callback(|kind, message| eprintln!("[glfw] {kind:?}: {message}"));
    window.set_size(960, 540)?;
    window.set_size_limits(Some(320), Some(240), Some(1920), Some(1080))?;
    window.set_aspect_ratio(Some((16, 9)))?;
    window.set_position(80, 80);
    window.opacity(0.96);
    println!(
        "size: {:?}, framebuffer: {:?}, scale: {:?}, position: {:?}, opacity: {}",
        window.get_size(),
        window.get_framebuffer_size(),
        window.get_content_scale(),
        window.get_position(),
        window.get_opacity(),
    );
    println!("Resize to observe 16:9. Press Space to clear the aspect ratio constraint.");

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Space) {
            window.set_aspect_ratio(None)?;
            window.set_title("VMNL window_config | aspect ratio cleared");
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }
    window.unset_error_callback();

    Ok(())
}
