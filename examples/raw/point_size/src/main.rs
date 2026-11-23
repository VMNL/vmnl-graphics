// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Render raw points larger than one pixel.

use vmnl_graphics::{raw, Context, DeviceConfig, DeviceFeature, VMNLResult, Window};

const VERT: &str = r"
    #version 450
    layout(location = 0) in vec2 position;
    layout(location = 1) in vec4 color;
    layout(location = 0) out vec4 vertex_color;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
        gl_PointSize = 10.0;
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
    let context = Context::builder()
        .device(DeviceConfig::default().require_features([DeviceFeature::LargePoints]))
        .build()?;
    let mut window = Window::builder()
        .title("VMNL raw_point_size")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let pipeline = spec
        .topology(raw::PrimitiveTopology::PointList)
        .build(&window)?;
    let triangle = raw::Geometry::builder([
        Vertex {
            position: [-0.65, -0.55],
            color: [1.0, 0.2, 0.2, 1.0],
        },
        Vertex {
            position: [0.65, -0.55],
            color: [0.2, 1.0, 0.2, 1.0],
        },
        Vertex {
            position: [0.0, 0.7],
            color: [0.2, 0.4, 1.0, 1.0],
        },
    ])
    .build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }
        window
            .render()
            .draw_raw_2d(&pipeline, [&triangle])
            .submit()?;
    }

    Ok(())
}
