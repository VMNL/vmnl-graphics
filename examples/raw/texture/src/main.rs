// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Compare raw texture bindings, filters, channel views, and blend presets.

use vmnl_graphics::{
    common::ColorBlendConfig,
    raw,
    texture::{
        AlphaMode, ColorEncoding, PixelSource, Sampler, SamplerConfig, Texture2D, TextureComponent,
        TextureComponentMapping, TextureFormat, TextureView2D, TextureViewInterpretation,
    },
    Context, Key, VMNLResult, Window,
};

const VERTEX_SHADER: &str = r"
    #version 450
    layout(location = 0) in vec2 position;
    layout(location = 1) in vec2 uv;
    layout(location = 0) out vec2 texture_uv;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
        texture_uv = uv;
    }
";

const COMBINED_FRAGMENT_SHADER: &str = r"
    #version 450
    layout(set = 0, binding = 0) uniform sampler2D color_image;
    layout(location = 0) in vec2 texture_uv;
    layout(location = 0) out vec4 out_color;
    void main() {
        out_color = texture(color_image, texture_uv);
    }
";

const COMBINED_PREMULTIPLIED_FRAGMENT_SHADER: &str = r"
    #version 450
    layout(set = 0, binding = 0) uniform sampler2D color_image;
    layout(location = 0) in vec2 texture_uv;
    layout(location = 0) out vec4 out_color;
    void main() {
        vec4 straight = texture(color_image, texture_uv);
        out_color = vec4(straight.rgb * straight.a, straight.a);
    }
";

const SEPARATE_FRAGMENT_SHADER: &str = r"
    #version 450
    layout(set = 0, binding = 0) uniform texture2D color_image;
    layout(set = 1, binding = 0) uniform sampler image_sampler;
    layout(location = 0) in vec2 texture_uv;
    layout(location = 0) out vec4 out_color;
    void main() {
        out_color = texture(sampler2D(color_image, image_sampler), texture_uv);
    }
";

const SEPARATE_PREMULTIPLIED_FRAGMENT_SHADER: &str = r"
    #version 450
    layout(set = 0, binding = 0) uniform texture2D color_image;
    layout(set = 1, binding = 0) uniform sampler image_sampler;
    layout(location = 0) in vec2 texture_uv;
    layout(location = 0) out vec4 out_color;
    void main() {
        vec4 straight = texture(sampler2D(color_image, image_sampler), texture_uv);
        out_color = vec4(straight.rgb * straight.a, straight.a);
    }
";

const BLEND_PRESETS: [BlendPreset; 3] = [
    BlendPreset::Opaque,
    BlendPreset::StraightAlpha,
    BlendPreset::PremultipliedAlpha,
];

#[repr(C)]
#[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
struct TextureVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32_SFLOAT)]
    uv: [f32; 2],
}

#[derive(Clone, Copy)]
enum BindingLayout {
    Combined,
    Separate,
}

#[derive(Clone, Copy)]
enum BlendPreset {
    Opaque,
    StraightAlpha,
    PremultipliedAlpha,
}

impl BlendPreset {
    const fn name(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::StraightAlpha => "straight alpha",
            Self::PremultipliedAlpha => "premultiplied alpha",
        }
    }

    const fn config(self) -> ColorBlendConfig {
        match self {
            Self::Opaque => ColorBlendConfig::opaque(),
            Self::StraightAlpha => ColorBlendConfig::straight_alpha(),
            Self::PremultipliedAlpha => ColorBlendConfig::premultiplied_alpha(),
        }
    }

    const fn uses_premultiplied_shader(self) -> bool {
        matches!(self, Self::PremultipliedAlpha)
    }
}

struct RenderVariant {
    pipeline: raw::Pipeline<TextureVertex>,
    resources: Vec<raw::Resources>,
}

#[derive(Default)]
struct Controls {
    linear: bool,
    swizzled: bool,
    blend_index: usize,
    previous_filter: bool,
    previous_view: bool,
    previous_blend: bool,
}

impl Controls {
    fn update(&mut self, window: &mut Window) -> bool {
        let (filter_pressed, view_pressed, blend_pressed, escape_pressed) = {
            let keyboard = window.input().keyboard();
            (
                keyboard.is_pressed(Key::F),
                keyboard.is_pressed(Key::V),
                keyboard.is_pressed(Key::B),
                keyboard.is_pressed(Key::Escape),
            )
        };
        let mut changed = false;

        if filter_pressed && !self.previous_filter {
            self.linear = !self.linear;
            changed = true;
        }
        if view_pressed && !self.previous_view {
            self.swizzled = !self.swizzled;
            changed = true;
        }
        if blend_pressed && !self.previous_blend {
            self.blend_index = (self.blend_index + 1) % BLEND_PRESETS.len();
            changed = true;
        }
        self.previous_filter = filter_pressed;
        self.previous_view = view_pressed;
        self.previous_blend = blend_pressed;

        if escape_pressed {
            window.close();
        }
        changed
    }

    fn resource_index(&self) -> usize {
        usize::from(self.swizzled) * 2 + usize::from(self.linear)
    }

    fn title(&self) -> String {
        format!(
            "VMNL raw_texture | {} | {} | {} (F/V/B)",
            if self.linear { "linear" } else { "nearest" },
            if self.swizzled {
                "red/blue swizzle"
            } else {
                "identity view"
            },
            BLEND_PRESETS[self.blend_index].name(),
        )
    }
}

fn build_variant(
    context: &Context,
    window: &Window,
    texture: &Texture2D,
    swizzled_view: &TextureView2D,
    samplers: &[Sampler; 2],
    binding_layout: BindingLayout,
    blend_preset: BlendPreset,
) -> VMNLResult<RenderVariant> {
    let separate = matches!(binding_layout, BindingLayout::Separate);
    let fragment_shader = match (separate, blend_preset.uses_premultiplied_shader()) {
        (false, false) => COMBINED_FRAGMENT_SHADER,
        (false, true) => COMBINED_PREMULTIPLIED_FRAGMENT_SHADER,
        (true, false) => SEPARATE_FRAGMENT_SHADER,
        (true, true) => SEPARATE_PREMULTIPLIED_FRAGMENT_SHADER,
    };
    let pipeline = raw::Pipeline::<TextureVertex>::builder()
        .vertex_shader(raw::ShaderSource::Src(VERTEX_SHADER.into()))
        .fragment_shader(raw::ShaderSource::Src(fragment_shader.into()))
        .topology(raw::PrimitiveTopology::TriangleList)
        .color_blend(blend_preset.config())
        .build(window)?;

    let mut resources = Vec::with_capacity(4);
    for view in [None, Some(swizzled_view)] {
        for sampler in samplers {
            let builder = raw::Resources::builder(&pipeline);
            let builder = match (binding_layout, view) {
                (BindingLayout::Combined, None) => builder.sampled_texture(0, 0, texture, sampler),
                (BindingLayout::Combined, Some(view)) => {
                    builder.sampled_texture_view(0, 0, view, sampler)
                }
                (BindingLayout::Separate, None) => {
                    builder.texture(0, 0, texture).sampler(1, 0, sampler)
                }
                (BindingLayout::Separate, Some(view)) => {
                    builder.texture_view(0, 0, view).sampler(1, 0, sampler)
                }
            };
            resources.push(builder.build(context)?);
        }
    }

    Ok(RenderVariant {
        pipeline,
        resources,
    })
}

fn panel_geometry(min_x: f32, max_x: f32) -> [TextureVertex; 6] {
    [
        TextureVertex {
            position: [min_x, -0.7],
            uv: [0.0, 0.0],
        },
        TextureVertex {
            position: [max_x, -0.7],
            uv: [1.0, 0.0],
        },
        TextureVertex {
            position: [max_x, 0.7],
            uv: [1.0, 1.0],
        },
        TextureVertex {
            position: [min_x, -0.7],
            uv: [0.0, 0.0],
        },
        TextureVertex {
            position: [max_x, 0.7],
            uv: [1.0, 1.0],
        },
        TextureVertex {
            position: [min_x, 0.7],
            uv: [0.0, 1.0],
        },
    ]
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL raw_texture")
        .size(1000, 600)
        .set_clear_color([24, 30, 42])
        .build(&context)?;

    let rgba = [
        255, 0, 0, 128, 0, 255, 0, 128, // top row: half-alpha red, green
        0, 0, 255, 128, 255, 255, 255, 128, // bottom row: blue, white
    ];
    let texture = Texture2D::from_pixels(
        &context,
        PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &rgba)?,
    )
    .build()?;
    let swizzled_view = TextureView2D::builder(&texture)
        .component_mapping(TextureComponentMapping::new(
            TextureComponent::Blue,
            TextureComponent::Green,
            TextureComponent::Red,
            TextureComponent::Alpha,
        ))
        .interpretation(TextureViewInterpretation::new(
            ColorEncoding::Linear,
            AlphaMode::Straight,
        ))
        .build()?;
    let samplers = [
        Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?,
        Sampler::builder(&context)
            .config(SamplerConfig::linear())
            .build()?,
    ];

    let mut variants = Vec::with_capacity(2 * BLEND_PRESETS.len());
    for binding_layout in [BindingLayout::Combined, BindingLayout::Separate] {
        for blend_preset in BLEND_PRESETS {
            variants.push(build_variant(
                &context,
                &window,
                &texture,
                &swizzled_view,
                &samplers,
                binding_layout,
                blend_preset,
            )?);
        }
    }

    let combined_geometry = raw::Geometry::builder(panel_geometry(-0.95, -0.05)).build(&context)?;
    let separate_geometry = raw::Geometry::builder(panel_geometry(0.05, 0.95)).build(&context)?;
    let mut controls = Controls::default();
    window.set_title(&controls.title());
    println!("Left: combined sampler2D binding; right: separate texture2D + sampler bindings.");
    println!(
        "F: toggle nearest/linear; V: toggle red/blue view; B: cycle blend preset; Escape: close."
    );

    while window.is_open() {
        for _ in window.poll_events() {}
        if controls.update(&mut window) {
            window.set_title(&controls.title());
        }
        let combined = &variants[controls.blend_index];
        let separate = &variants[BLEND_PRESETS.len() + controls.blend_index];
        let resource_index = controls.resource_index();
        window
            .render()
            .draw_raw_2d_with(
                &combined.pipeline,
                &combined.resources[resource_index],
                [&combined_geometry],
            )
            .draw_raw_2d_with(
                &separate.pipeline,
                &separate.resources[resource_index],
                [&separate_geometry],
            )
            .submit()?;
    }

    Ok(())
}
