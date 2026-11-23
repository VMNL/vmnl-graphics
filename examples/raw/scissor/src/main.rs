// SPDX-FileCopyrightText: 2026 VMNL contributors
// SPDX-License-Identifier: MIT

//! Compare full-framebuffer and fixed scissor with the same geometry.

use vmnl_graphics::{raw, Context, Key, PresentMode, VMNLErrorKind, VMNLResult, Window};

const VERT: &str = r#"
#version 460

layout(location = 0) in vec2 position;
layout(location = 1) in vec3 color;

layout(location = 0) out vec3 out_color;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    out_color = color;
}
"#;

const FRAG: &str = r#"
#version 460

layout(location = 0) in vec3 in_color;
layout(location = 0) out vec4 out_color;

void main() {
    out_color = vec4(in_color, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
struct RawVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32_SFLOAT)]
    color: [f32; 3],
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL raw_scissor | full framebuffer (Space: toggle)")
        .size(900, 600)
        .set_clear_color([0, 0, 0, 255])
        .present_mode(PresentMode::Auto)
        .build(&context)?;

    let spec = raw::Pipeline::<RawVertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()))
        .topology(raw::PrimitiveTopology::TriangleList)
        .blend_mode(raw::BlendMode::Opaque);
    let pipelines = [
        spec.clone().build(&window)?,
        spec.scissor(raw::ScissorPolicy::Fixed(raw::Scissor {
            offset: [375, 260],
            extent: [150, 115],
        }))
        .build(&window)?,
    ];

    let geometry = raw::Geometry::builder([
        RawVertex {
            position: [-0.6, -0.5],
            color: [1.0, 0.0, 0.0],
        },
        RawVertex {
            position: [0.6, -0.5],
            color: [0.0, 1.0, 0.0],
        },
        RawVertex {
            position: [0.0, 0.6],
            color: [0.0, 0.0, 1.0],
        },
    ])
    .build(&context)?;

    let mut fixed = false;
    let mut displayed_size = None;
    while window.is_open() {
        for _ in window.poll_events() {}
        let toggle = window.input().keyboard().is_pressed(Key::Space);
        if toggle {
            fixed = !fixed;
        }
        let size = window.get_framebuffer_size();
        if toggle || displayed_size != Some(size) {
            displayed_size = Some(size);
            let (width, height) = size;
            let policy = if fixed {
                "fixed [375,260] 150x115"
            } else {
                "full framebuffer"
            };
            window.set_title(&format!(
                "VMNL raw_scissor | {policy} | {width}x{height} (Space: toggle)"
            ));
        }
        let result = window
            .render()
            .draw_raw_2d(&pipelines[usize::from(fixed)], [&geometry])
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
