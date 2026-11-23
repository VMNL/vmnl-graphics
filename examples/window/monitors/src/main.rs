// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Inspect monitor names, work areas and primary monitor selection.

use vmnl_graphics::{Context, Key, VMNLResult, Window};

fn print_monitor_summary(window: &Window) {
    let monitor_names = window
        .monitor()
        .names()
        .into_iter()
        .map(|name| name.unwrap_or_else(|| "Unknown".to_string()))
        .collect::<Vec<_>>();

    println!("monitors: {}", monitor_names.join(", "));
    if let Some(primary) = window.monitor().primary() {
        println!(
            "primary: name={:?} pos={:?} workarea={:?} scale={:?}",
            primary.name, primary.position, primary.workarea, primary.content_scale
        );
    }
    println!("monitor count: {}", window.monitor().infos().len());
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL window_monitors")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    print_monitor_summary(&window);
    println!("Press Space to list monitors again.");

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Space) {
            print_monitor_summary(&window);
        }
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
