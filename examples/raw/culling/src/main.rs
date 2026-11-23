// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare cull modes and front face for two opposite windings.

use vmnl_graphics::{raw, Context, Key, VMNLResult, Window};

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
        .title("VMNL raw_culling")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let modes = [
        raw::CullMode::None,
        raw::CullMode::Front,
        raw::CullMode::Back,
        raw::CullMode::FrontAndBack,
    ];
    let faces = [raw::FrontFace::CounterClockwise, raw::FrontFace::Clockwise];
    let mut pipelines = Vec::new();
    for face in faces {
        for mode in modes {
            pipelines.push(
                spec.clone()
                    .front_face(face)
                    .cull_mode(mode)
                    .build(&window)?,
            );
        }
    }
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
        .indices([0, 1, 2, 0, 3, 2])
        .build(&context)?;
    let mut mode = 0;
    let mut face = 0;
    window.set_title("VMNL raw_culling | None / CounterClockwise (C: cull, F: front face)");

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        let keyboard = window.input().keyboard();
        let cull = keyboard.is_pressed(Key::C);
        let winding = keyboard.is_pressed(Key::F);
        if cull {
            mode = (mode + 1) % modes.len();
        }
        if winding {
            face ^= 1;
        }
        if cull || winding {
            window.set_title(&format!(
                "VMNL raw_culling | {:?} / {:?} (C: cull, F: front face)",
                modes[mode], faces[face]
            ));
        }
        window
            .render()
            .draw_raw_2d(&pipelines[face * modes.len() + mode], [&geometry])
            .submit()?;
    }

    Ok(())
}
