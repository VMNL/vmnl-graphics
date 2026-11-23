// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare depth test and writes by reversing two coincident draws.

use vmnl_graphics::{
    raw, Context, DepthFormat, Key, RenderTargetConfig, VMNLError, VMNLErrorKind, VMNLResult,
    Window,
};

const VERT: &str = r"
    #version 450
    layout(location = 0) in vec3 position;
    layout(location = 1) in vec4 color;
    layout(location = 0) out vec4 vertex_color;
    void main() {
        gl_Position = vec4(position, 1.0);
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
    #[format(R32G32B32_SFLOAT)]
    position: [f32; 3],
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let format = match std::env::var("VMNL_DEPTH_FORMAT").as_deref() {
        Ok("d32") => DepthFormat::D32Sfloat,
        Ok("d16") | Err(std::env::VarError::NotPresent) => DepthFormat::D16Unorm,
        _ => {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "VMNL_DEPTH_FORMAT must be d16 or d32".into(),
            )))
        }
    };
    let mut window = Window::builder()
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .render_target(RenderTargetConfig::default().depth(format))
        .build(&context)?;
    println!("Resolved target: {:?}", window.render_target_info());
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let states = [
        raw::DepthState::Disabled,
        raw::DepthState::Test {
            compare_op: raw::CompareOp::Less,
            write_enable: true,
        },
        raw::DepthState::Test {
            compare_op: raw::CompareOp::Less,
            write_enable: false,
        },
    ];
    let pipelines = states
        .into_iter()
        .map(|state| spec.clone().depth(state).build(&window))
        .collect::<VMNLResult<Vec<_>>>()?;
    let triangle = |z, color| {
        [
            Vertex {
                position: [-0.65, -0.55, z],
                color,
            },
            Vertex {
                position: [0.65, -0.55, z],
                color,
            },
            Vertex {
                position: [0.0, 0.7, z],
                color,
            },
        ]
    };
    let near = raw::Geometry::builder(triangle(0.25, [1.0, 0.85, 0.1, 1.0])).build(&context)?;
    let far = raw::Geometry::builder(triangle(0.75, [0.85, 0.15, 0.9, 1.0])).build(&context)?;
    let mut selected = 0;
    let mut far_first = false;
    window.set_title("VMNL raw_depth | disabled / near first (D: depth, O: order)");

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        let keyboard = window.input().keyboard();
        let change_depth = keyboard.is_pressed(Key::D);
        let change_order = keyboard.is_pressed(Key::O);
        if change_depth {
            selected = (selected + 1) % states.len();
        }
        if change_order {
            far_first = !far_first;
        }
        if change_depth || change_order {
            let order = if far_first { "far first" } else { "near first" };
            window.set_title(&format!(
                "VMNL raw_depth | {:?} / {order} (D: depth, O: order)",
                states[selected]
            ));
        }

        let triangles = if far_first {
            [&far, &near]
        } else {
            [&near, &far]
        };
        let result = window
            .render()
            .draw_raw_2d(&pipelines[selected], triangles)
            .submit();
        match result {
            Ok(()) => {}
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => {
                window.wait_events_timeout(0.05);
            }
            Err(error) => return Err(error),
        }
    }

    Ok(())
}
