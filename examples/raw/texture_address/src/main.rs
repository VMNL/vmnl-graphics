// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare texture addressing outside the normalized U range.

use vmnl_graphics::{
    raw,
    texture::{
        AddressMode, PixelSource, Sampler, SamplerBorderColor, SamplerConfig, Texture2D,
        TextureFormat,
    },
    Context, Key, VMNLResult, Window,
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

fn panel_geometry() -> [TextureVertex; 6] {
    [
        TextureVertex {
            position: [-0.85, -0.7],
            uv: [-0.75, 0.0],
        },
        TextureVertex {
            position: [0.85, -0.7],
            uv: [1.75, 0.0],
        },
        TextureVertex {
            position: [0.85, 0.7],
            uv: [1.75, 1.0],
        },
        TextureVertex {
            position: [-0.85, -0.7],
            uv: [-0.75, 0.0],
        },
        TextureVertex {
            position: [0.85, 0.7],
            uv: [1.75, 1.0],
        },
        TextureVertex {
            position: [-0.85, 0.7],
            uv: [-0.75, 1.0],
        },
    ]
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL raw_texture_address | ClampToEdge (Space: cycle)")
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
    let pipeline = raw::Pipeline::<TextureVertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERTEX_SHADER.into()))
        .fragment_shader(raw::ShaderSource::Src(COMBINED_FRAGMENT_SHADER.into()))
        .build(&window)?;

    let modes = [
        AddressMode::ClampToEdge,
        AddressMode::Repeat,
        AddressMode::MirroredRepeat,
        AddressMode::ClampToBorder,
    ];
    let resources = modes
        .iter()
        .map(|&mode| {
            let sampler = Sampler::builder(&context)
                .config(
                    SamplerConfig::nearest()
                        .with_address_u(mode)
                        .with_border_color(SamplerBorderColor::FloatOpaqueBlack),
                )
                .build()?;
            raw::Resources::builder(&pipeline)
                .sampled_texture(0, 0, &texture, &sampler)
                .build(&context)
        })
        .collect::<VMNLResult<Vec<_>>>()?;
    let panel = raw::Geometry::builder(panel_geometry()).build(&context)?;

    let mut selected = 0;
    while window.is_open() {
        for _ in window.poll_events() {}
        if window.input().keyboard().is_pressed(Key::Space) {
            selected = (selected + 1) % modes.len();
            window.set_title(&format!(
                "VMNL raw_texture_address | {:?} (Space: cycle)",
                modes[selected]
            ));
        }
        if window.is_open() {
            window
                .render()
                .draw_raw_2d_with(&pipeline, &resources[selected], [&panel])
                .submit()?;
        }
    }

    Ok(())
}
