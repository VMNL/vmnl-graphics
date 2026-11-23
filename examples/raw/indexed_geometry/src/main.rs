// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Draw an indexed quad from four raw vertices.

use vmnl_graphics::{raw, Context, VMNLResult, Window};

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
        .title("VMNL raw_indexed_geometry")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let pipeline = spec.build(&window)?;
    let vertices = [
        Vertex {
            position: [-0.6, -0.5],
            color: [1.0, 0.2, 0.2, 1.0],
        },
        Vertex {
            position: [0.6, -0.5],
            color: [0.2, 1.0, 0.2, 1.0],
        },
        Vertex {
            position: [0.6, 0.5],
            color: [0.2, 0.4, 1.0, 1.0],
        },
        Vertex {
            position: [-0.6, 0.5],
            color: [1.0, 1.0, 0.2, 1.0],
        },
    ];
    let quad = raw::Geometry::builder(vertices)
        .indices([0, 1, 2, 0, 2, 3])
        .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }
        window.render().draw_raw_2d(&pipeline, [&quad]).submit()?;
    }

    Ok(())
}
