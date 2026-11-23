// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare unit width with a second line width supported by the device.

use vmnl_graphics::{
    raw, Context, DeviceConfig, DeviceFeature, VMNLError, VMNLErrorKind, VMNLResult, Window,
};

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
        .device(DeviceConfig::default().require_features([DeviceFeature::WideLines]))
        .build()?;
    let mut window = Window::builder()
        .title("VMNL raw_line_width")
        .size(800, 600)
        .set_clear_color([12, 16, 24, 255])
        .build(&context)?;
    let spec = raw::Pipeline::<Vertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERT.into()))
        .fragment_shader(raw::ShaderSource::Src(FRAG.into()));
    let limits = context.line_width_limits();
    let wide = limits.max.min(8.0);
    if wide <= 1.0 {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
            "the selected GPU exposes no line width above 1".into(),
        )));
    }
    let spec = spec.topology(raw::PrimitiveTopology::LineList);
    let thin = spec.clone().line_width(1.0).build(&window)?;
    let wide = spec.line_width(wide).build(&window)?;
    println!(
        "Device limits: {limits:?}. Right pipeline requests width {}.",
        wide.line_width_value()
    );
    let vertices = |x: f32| {
        [
            Vertex {
                position: [x, -0.65],
                color: [0.2, 0.8, 1.0, 1.0],
            },
            Vertex {
                position: [x, 0.65],
                color: [0.2, 0.8, 1.0, 1.0],
            },
        ]
    };
    let left = raw::Geometry::builder(vertices(-0.4)).build(&context)?;
    let right = raw::Geometry::builder(vertices(0.4)).build(&context)?;

    while window.is_open() {
        for _ in window.poll_events() {}
        if !window.is_open() {
            break;
        }

        window
            .render()
            .draw_raw_2d(&thin, [&left])
            .draw_raw_2d(&wide, [&right])
            .submit()?;
    }

    Ok(())
}
