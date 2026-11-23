// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Assign standard and custom cursors, then restore the backend default.

use vmnl_graphics::{Context, Cursor, Key, StandardCursor, VMNLResult, Window};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL input_cursor")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    let standard_cursor = Cursor::standard(StandardCursor::PointingHand).build(&context)?;
    let (width, height) = (32_u32, 32_u32);
    let mut custom_pixels = vec![0_u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            if x == y || x + y == width - 1 {
                let offset = ((y * width + x) * 4) as usize;
                custom_pixels[offset..offset + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
    }

    let custom_cursor = Cursor::rgba8(width, height, &custom_pixels)
        .hotspot(16, 16)
        .hotspot_marker([0, 255, 0, 255])
        .build(&context)?;
    window.set_cursor(Some(&standard_cursor))?;

    println!("S selects the standard cursor, U selects a custom cursor, O restores the default.");
    while window.is_open() {
        for _ in window.poll_events() {}
        let keyboard = window.input().keyboard();
        let standard = keyboard.is_pressed(Key::S);
        let custom = keyboard.is_pressed(Key::U);
        let reset = keyboard.is_pressed(Key::O);
        if standard {
            window.set_cursor(Some(&standard_cursor))?;
            window.set_title("VMNL input_cursor | standard");
        }
        if custom {
            window.set_cursor(Some(&custom_cursor))?;
            window.set_title("VMNL input_cursor | custom cross");
        }
        if reset {
            window.set_cursor(None)?;
            window.set_title("VMNL input_cursor | backend default");
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
