// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Build a raw pipeline using shader files.

use vmnl_graphics::{raw, Context, VMNLResult, Window};

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
        .title("VMNL raw_shader_path")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let shader_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Path(shader_dir.join("raw.vert")))
        .fragment_shader(raw::ShaderSource::Path(shader_dir.join("raw.frag")));
    let pipeline = spec.build(&window)?;
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
