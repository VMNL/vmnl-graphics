// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare full-framebuffer and fixed viewport transforms.

use vmnl_graphics::{raw, Context, Key, VMNLErrorKind, VMNLResult, Window};

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
        .title("VMNL raw_viewport")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let pipelines = [
        spec.clone().build(&window)?,
        spec.viewport(raw::ViewportPolicy::Fixed(raw::Viewport {
            offset: [100.0, 75.0],
            extent: [400.0, 300.0],
            depth_range: [0.0, 1.0],
        }))
        .build(&window)?,
    ];
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
    let mut fixed = false;
    let mut displayed_size = None;
    window.set_title("VMNL raw_viewport | full framebuffer (Space: toggle)");

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        let toggle = window.input().keyboard().is_pressed(Key::Space);
        if toggle {
            fixed = !fixed;
        }
        let size = window.get_framebuffer_size();
        if toggle || displayed_size != Some(size) {
            displayed_size = Some(size);
            let (width, height) = size;
            let mode = if fixed {
                "fixed [100,75] 400x300"
            } else {
                "full framebuffer"
            };
            window.set_title(&format!(
                "VMNL raw_viewport | {mode} | {width}x{height} (Space: toggle)"
            ));
        }
        let result = window
            .render()
            .draw_raw_2d(&pipelines[usize::from(fixed)], [&triangle])
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
