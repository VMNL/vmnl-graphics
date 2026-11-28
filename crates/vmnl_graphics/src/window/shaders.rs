// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Shader definitions and default GLSL sources for the VMNL window pipeline.

use crate::common::ShaderSource;

/// Struct to hold shader inputs for a window, allowing for dynamic shader management.
#[derive(Debug, Clone)]
pub(crate) struct WindowShaders {
    /// Optional vertex shader input.
    pub vertex: Option<ShaderSource>,
    /// Optional fragment shader input.
    pub fragment: Option<ShaderSource>,
}

pub(crate) const DEFAULT_VERTEX_SHADER: &str = r"
    #version 460

    layout(push_constant) uniform PushConstants {
        vec2 window_size;
    } pc;

    layout(location = 0) in vec2 position;
    layout(location = 1) in vec4 color;
    layout(location = 0) out vec4 out_color;

    void main() {
        vec2 ndc = vec2(
            (2.0 * position.x / pc.window_size.x) - 1.0,
            (2.0 * position.y / pc.window_size.y) - 1.0
        );

        gl_Position = vec4(ndc, 0.0, 1.0);
        out_color = color;
    }
";

pub(crate) const DEFAULT_FRAGMENT_SHADER: &str = r"
    #version 460

    layout(location = 0) in vec4 in_color;
    layout(location = 0) out vec4 f_color;

    void main() {
        f_color = in_color;
    }
";

pub(crate) const TEXTURED_VERTEX_SHADER: &str = r"
    #version 460

    layout(push_constant) uniform TexturedDraw {
        vec2 window_size;
        uint alpha_mode;
    } draw;

    layout(location = 0) in vec2 position;
    layout(location = 1) in vec4 modulation;
    layout(location = 2) in vec2 uv;
    layout(location = 0) out vec4 out_modulation;
    layout(location = 1) out vec2 out_uv;

    void main() {
        gl_Position = vec4(2.0 * position / draw.window_size - 1.0, 0.0, 1.0);
        out_modulation = modulation;
        out_uv = uv;
    }
";

pub(crate) const TEXTURED_FRAGMENT_SHADER: &str = r"
    #version 460

    layout(push_constant) uniform TexturedDraw {
        vec2 window_size;
        uint alpha_mode;
    } draw;

    layout(set = 0, binding = 0) uniform sampler2D source_texture;
    layout(location = 0) in vec4 modulation;
    layout(location = 1) in vec2 uv;
    layout(location = 0) out vec4 color;

    void main() {
        vec4 sample_color = texture(source_texture, uv);
        // 0 is straight, 1 is premultiplied, 2 is opaque.
        vec3 rgb = sample_color.rgb * modulation.rgb;
        if (draw.alpha_mode == 1) {
            rgb *= modulation.a;
        }
        float source_alpha = draw.alpha_mode == 2 ? 1.0 : sample_color.a;
        color = vec4(rgb, source_alpha * modulation.a);
    }
";
