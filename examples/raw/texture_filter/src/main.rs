// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare nearest and linear filtering of the same four texels.

use vmnl_graphics::{
    raw,
    texture::{PixelSource, Sampler, SamplerConfig, Texture2D, TextureFormat},
    Context, VMNLResult, Window,
};

const VERTEX_SHADER: &str = r"
    #version 450
    layout(location = 0) in vec2 position;
    layout(location = 1) in vec2 uv;
    layout(location = 0) out vec2 texture_uv;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
        texture_uv = uv;
    }
";

const COMBINED_FRAGMENT_SHADER: &str = r"
    #version 450
    layout(set = 0, binding = 0) uniform sampler2D color_image;
    layout(location = 0) in vec2 texture_uv;
    layout(location = 0) out vec4 out_color;
    void main() {
        out_color = texture(color_image, texture_uv);
    }
";

#[repr(C)]
#[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
struct TextureVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32_SFLOAT)]
    uv: [f32; 2],
}

fn panel_geometry(min_x: f32, max_x: f32) -> [TextureVertex; 6] {
    [
        TextureVertex {
            position: [min_x, -0.7],
            uv: [0.0, 0.0],
        },
        TextureVertex {
            position: [max_x, -0.7],
            uv: [1.0, 0.0],
        },
        TextureVertex {
            position: [max_x, 0.7],
            uv: [1.0, 1.0],
        },
        TextureVertex {
            position: [min_x, -0.7],
            uv: [0.0, 0.0],
        },
        TextureVertex {
            position: [max_x, 0.7],
            uv: [1.0, 1.0],
        },
        TextureVertex {
            position: [min_x, 0.7],
            uv: [0.0, 1.0],
        },
    ]
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL raw_texture_filter")
        .size(900, 600)
        .set_clear_color([24, 30, 42])
        .build(&context)?;

    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, // top row: red, green
        0, 0, 255, 255, 255, 255, 255, 255, // bottom row: blue, white
    ];
    let texture = Texture2D::from_pixels(
        &context,
        PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &rgba)?,
    )
    .build()?;
    let sampler = Sampler::builder(&context)
        .config(SamplerConfig::nearest())
        .build()?;
    let pipeline = raw::Pipeline::<TextureVertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERTEX_SHADER.into()))
        .fragment_shader(raw::ShaderSource::Src(COMBINED_FRAGMENT_SHADER.into()))
        .build(&window)?;

    let linear_sampler = Sampler::builder(&context)
        .config(SamplerConfig::linear())
        .build()?;
    let left_resources = raw::Resources::builder(&pipeline)
        .sampled_texture(0, 0, &texture, &sampler)
        .build(&context)?;
    let right_resources = raw::Resources::builder(&pipeline)
        .sampled_texture(0, 0, &texture, &linear_sampler)
        .build(&context)?;
    window.set_title("VMNL raw_texture_filter | nearest (left) / linear (right)");
    let left = raw::Geometry::builder(panel_geometry(-0.95, -0.05)).build(&context)?;
    let right = raw::Geometry::builder(panel_geometry(0.05, 0.95)).build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw_raw_2d_with(&pipeline, &left_resources, [&left])
                .draw_raw_2d_with(&pipeline, &right_resources, [&right])
                .submit()?;
        }
    }

    Ok(())
}
