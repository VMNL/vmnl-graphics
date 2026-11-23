// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare fill and wireframe for a quad containing two triangles.

use vmnl_graphics::{raw, Context, DeviceConfig, DeviceFeature, Key, VMNLResult, Window};

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
    let context = Context::builder()
        .device(DeviceConfig::default().require_features([DeviceFeature::FillModeNonSolid]))
        .build()?;
    let mut window = Window::builder()
        .title("VMNL raw_wireframe")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let pipelines = [
        spec.clone()
            .polygon_mode(raw::PolygonMode::Fill)
            .build(&window)?,
        spec.polygon_mode(raw::PolygonMode::Line).build(&window)?,
    ];
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
    let geometry = raw::Geometry::builder(vertices)
        .indices([0, 1, 2, 0, 2, 3])
        .build(&context)?;
    let mut wireframe = false;
    window.set_title("VMNL raw_wireframe | fill (Space: toggle)");

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        if window.input().keyboard().is_pressed(Key::Space) {
            wireframe = !wireframe;
            let mode = if wireframe { "wireframe" } else { "fill" };
            window.set_title(&format!("VMNL raw_wireframe | {mode} (Space: toggle)"));
        }
        window
            .render()
            .draw_raw_2d(&pipelines[usize::from(wireframe)], [&geometry])
            .submit()?;
    }

    Ok(())
}
