// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Load a PNG file and display its four texels.

use vmnl_graphics::{
    raw,
    texture::{Sampler, SamplerConfig, Texture2D},
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
        .title("VMNL raw_texture_png")
        .size(900, 600)
        .set_clear_color([24, 30, 42])
        .build(&context)?;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/texels.png");
    let texture = Texture2D::from_file(&context, path).build()?;
    let sampler = Sampler::builder(&context)
        .config(SamplerConfig::nearest())
        .build()?;
    let pipeline = raw::Pipeline::<TextureVertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERTEX_SHADER.into()))
        .fragment_shader(raw::ShaderSource::Src(COMBINED_FRAGMENT_SHADER.into()))
        .build(&window)?;

    let resources = raw::Resources::builder(&pipeline)
        .sampled_texture(0, 0, &texture, &sampler)
        .build(&context)?;
    let quad = raw::Geometry::builder(panel_geometry(-0.6, 0.6)).build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if window.is_open() {
            window
                .render()
                .draw_raw_2d_with(&pipeline, &resources, [&quad])
                .submit()?;
        }
    }

    Ok(())
}
