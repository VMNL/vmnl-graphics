// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Select a surface color pair and keep it while resizing.

use vmnl_graphics::{
    d2::Shape, ColorFormat, Context, Key, SurfaceColorConfig, SurfaceColorFallback,
    SurfaceColorPair, SurfaceColorSpace, VMNLErrorKind, VMNLResult, Window,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let space = SurfaceColorSpace::SRGB_NON_LINEAR;
    let policy = if std::env::var_os("VMNL_SURFACE_COLOR_UNORM").is_some() {
        SurfaceColorConfig::exact(ColorFormat::B8G8R8A8_UNORM, space)
    } else {
        SurfaceColorConfig::prefer_with_fallback(
            [
                SurfaceColorPair::new(ColorFormat::B8G8R8A8_SRGB, space),
                SurfaceColorPair::new(ColorFormat::R8G8B8A8_SRGB, space),
            ],
            SurfaceColorFallback::FirstReported,
        )?
    };
    let mut window = Window::builder()
        .size(640, 480)
        .surface_color(policy)
        .set_clear_color([128, 128, 128])
        .build(&context)?;
    let info = window.surface_color_info();
    let pair = info.pair();
    let title = format!(
        "{} / {} ({:?})",
        pair.format(),
        pair.color_space(),
        info.selection()
    );
    window.set_title(&title);
    println!("Selected {title}");
    for pair in window.surface_color_pairs()? {
        println!("Supported {} / {}", pair.format(), pair.color_space());
    }

    let shape = Shape::rect(240.0, 150.0)
        .position(100.0, 100.0)
        .color([28, 120, 230, 150])
        .build(&context)?;
    println!(
        "Resize and check the rectangle still renders with the same reported pair. Escape closes."
    );
    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Escape) {
            break;
        }
        match window.render().draw2d([&shape]).submit() {
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => (),
            result => result?,
        }
    }
    Ok(())
}
