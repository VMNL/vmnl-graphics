// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare primitive assembly with an unchanged stream of vertices.

use vmnl_graphics::{raw, Context, Key, VMNLResult, Window};

const VERT: &str = r"
    #version 450
    layout(location = 0) in vec2 position;
    layout(location = 1) in vec4 color;
    layout(location = 0) out vec4 vertex_color;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
        gl_PointSize = 1.0;
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
        .title("VMNL raw_topology")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    use raw::PrimitiveTopology::{LineList, LineStrip, PointList, TriangleList, TriangleStrip};

    let topologies = [TriangleList, TriangleStrip, LineList, LineStrip, PointList];
    let pipelines = topologies
        .iter()
        .map(|&topology| spec.clone().topology(topology).build(&window))
        .collect::<VMNLResult<Vec<_>>>()?;
    let geometry = raw::Geometry::builder([
        Vertex {
            position: [-0.7, -0.5],
            color: [1.0, 0.2, 0.2, 1.0],
        },
        Vertex {
            position: [-0.7, 0.5],
            color: [0.2, 1.0, 0.2, 1.0],
        },
        Vertex {
            position: [0.0, -0.5],
            color: [0.2, 0.4, 1.0, 1.0],
        },
        Vertex {
            position: [0.0, 0.5],
            color: [1.0, 1.0, 0.2, 1.0],
        },
        Vertex {
            position: [0.7, -0.5],
            color: [0.2, 0.8, 1.0, 1.0],
        },
        Vertex {
            position: [0.7, 0.5],
            color: [1.0, 0.4, 0.8, 1.0],
        },
    ])
    .build(&context)?;
    let mut selected = 0;
    window.set_title("VMNL raw_topology | TriangleList (Space: cycle)");

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        if window.input().keyboard().is_pressed(Key::Space) {
            selected = (selected + 1) % topologies.len();
            window.set_title(&format!(
                "VMNL raw_topology | {:?} (Space: cycle)",
                topologies[selected]
            ));
        }
        window
            .render()
            .draw_raw_2d(&pipelines[selected], [&geometry])
            .submit()?;
    }

    Ok(())
}
