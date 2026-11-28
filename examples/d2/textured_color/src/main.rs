// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare straight and premultiplied sRGB textures with the same linear tint.

use vmnl_graphics::{
    common::Rgba,
    d2::{Shape, TexturedShape, TexturedVertex2D, Vector2f},
    texture::{AlphaMode, PixelSource, Sampler, SamplerConfig, Texture2D, TextureFormat},
    ColorFormat, Context, Key, SurfaceColorConfig, SurfaceColorPair, SurfaceColorSpace,
    VMNLErrorKind, VMNLResult, Window,
};

fn panel(
    context: &Context,
    window: &Window,
    sampler: &Sampler,
    left: f32,
    pixels: &[u8],
    alpha: AlphaMode,
) -> VMNLResult<TexturedShape> {
    let source = PixelSource::new(2, 2, TextureFormat::Rgba8Srgb, pixels)?.with_alpha_mode(alpha);
    let texture = Texture2D::from_pixels(context, source).build()?;
    let vertex = |x, y, uv| TexturedVertex2D {
        position: Vector2f { x, y },
        color: Rgba::WHITE,
        uv,
    };
    Shape::indexed_textured(
        [
            vertex(left, 50.0, [0.0, 0.0]),
            vertex(left + 240.0, 50.0, [1.0, 0.0]),
            vertex(left, 290.0, [0.0, 1.0]),
            vertex(left + 240.0, 290.0, [1.0, 1.0]),
        ],
        [0, 1, 2, 2, 1, 3],
        &texture.view(),
    )
    .sampler(sampler)
    .tint([224, 160, 255, 192])
    .build(window)
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let space = SurfaceColorSpace::SRGB_NON_LINEAR;
    let target = SurfaceColorConfig::prefer([
        SurfaceColorPair::new(ColorFormat::B8G8R8A8_SRGB, space),
        SurfaceColorPair::new(ColorFormat::R8G8B8A8_SRGB, space),
    ])?;
    let mut window = Window::builder()
        .size(640, 340)
        .title("Textured 2D | straight (left), premultiplied (right)")
        .surface_color(target)
        .build(&context)?;
    let pair = window.surface_color_info().pair();
    println!(
        "Target {} / {}. Resize and compare the panels. Escape closes.",
        pair.format(),
        pair.color_space()
    );

    let sampler = Sampler::builder(&context)
        .config(SamplerConfig::nearest())
        .build()?;
    let straight = [
        255, 0, 0, 128, 0, 255, 0, 128, 0, 0, 255, 128, 255, 255, 255, 128,
    ];
    // Byte 188 encodes about half intensity in sRGB. Premultiplication is in linear light.
    let premultiplied = [
        188, 0, 0, 128, 0, 188, 0, 128, 0, 0, 188, 128, 188, 188, 188, 128,
    ];
    let left = panel(
        &context,
        &window,
        &sampler,
        50.0,
        &straight,
        AlphaMode::Straight,
    )?;
    let right = panel(
        &context,
        &window,
        &sampler,
        350.0,
        &premultiplied,
        AlphaMode::Premultiplied,
    )?;
    let backdrop = Shape::rect(580.0, 120.0)
        .position(30.0, 170.0)
        .color([255, 255, 255, 255])
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Escape) {
            break;
        }
        match window
            .render()
            .draw2d([&backdrop])
            .draw2d([&left, &right])
            .submit()
        {
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => (),
            result => result?,
        }
    }
    Ok(())
}
