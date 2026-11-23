// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare opaque, straight-alpha and premultiplied-alpha blending.

use vmnl_graphics::{common::ColorBlendConfig, raw, Context, VMNLResult, Window};

const VERT: &str = r"
    #version 450
    layout(location = 0) in vec2 position;
    layout(location = 1) in vec4 color;
    layout(location = 0) out vec4 vertex_color;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
        vertex_color = color;
    }
";

const FRAG: &str = r"
    #version 450
    layout(location = 0) in vec4 vertex_color;
    layout(location = 0) out vec4 out_color;
    void main() {
        out_color = vertex_color;
    }
";

#[repr(C)]
#[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
struct Vertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL raw_blend")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let opaque = spec
        .clone()
        .color_blend(ColorBlendConfig::opaque())
        .build(&window)?;
    let straight = spec
        .clone()
        .color_blend(ColorBlendConfig::straight_alpha())
        .build(&window)?;
    let premultiplied = spec
        .color_blend(ColorBlendConfig::premultiplied_alpha())
        .build(&window)?;
    let triangle = |x: f32, color| {
        [
            Vertex {
                position: [x - 0.22, -0.45],
                color,
            },
            Vertex {
                position: [x + 0.22, -0.45],
                color,
            },
            Vertex {
                position: [x, 0.45],
                color,
            },
        ]
    };
    let rgba = [1.0, 0.4, 0.1, 0.5];
    let left = raw::Geometry::builder(triangle(-0.6, rgba)).build(&context)?;
    let middle = raw::Geometry::builder(triangle(0.0, rgba)).build(&context)?;
    let right = raw::Geometry::builder(triangle(0.6, [0.5, 0.2, 0.05, 0.5])).build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        window
            .render()
            .draw_raw_2d(&opaque, [&left])
            .draw_raw_2d(&straight, [&middle])
            .draw_raw_2d(&premultiplied, [&right])
            .submit()?;
    }

    Ok(())
}
