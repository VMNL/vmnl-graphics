// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use vmnl_graphics::{
    common::{BufferMemoryPreference, ColorBlendConfig, ColorBlendFactor, ColorBlendOperation},
    raw, PresentMode, RenderMode, VMNLResult,
};

#[derive(Clone)]
struct RawSpecVertex;

#[test]
fn raw_pipeline_spec_preserves_face_defaults() -> VMNLResult<()> {
    let spec = raw::Pipeline::<RawSpecVertex>::builder();
    assert_eq!(spec.cull_mode_value(), raw::CullMode::None);
    assert_eq!(spec.front_face_value(), raw::FrontFace::CounterClockwise);
    assert_eq!(spec.polygon_mode_value(), raw::PolygonMode::Fill);
    assert_eq!(spec.line_width_value(), 1.0);

    Ok(())
}

#[test]
fn raw_pipeline_spec_preserves_and_replaces_rasterization_values() {
    for mode in [
        raw::PolygonMode::Fill,
        raw::PolygonMode::Line,
        raw::PolygonMode::Point,
    ] {
        let spec = raw::Pipeline::<RawSpecVertex>::builder()
            .polygon_mode(mode)
            .line_width(2.5);
        assert_eq!(spec.polygon_mode_value(), mode);
        assert_eq!(spec.line_width_value(), 2.5);
        assert_eq!(spec.topology_value(), raw::PrimitiveTopology::TriangleList);
        assert_eq!(spec.cull_mode_value(), raw::CullMode::None);
        let spec = spec
            .clone()
            .polygon_mode(raw::PolygonMode::Fill)
            .line_width(1.0);
        assert_eq!(spec.polygon_mode_value(), raw::PolygonMode::Fill);
        assert_eq!(spec.line_width_value(), 1.0);
    }
    // Numeric validation belongs to build, where the device limits are known.
    let spec = raw::Pipeline::<RawSpecVertex>::builder().line_width(f32::NAN);
    assert!(spec.line_width_value().is_nan());
}

#[test]
fn line_width_limits_and_errors_are_backend_independent() {
    let limits = vmnl_graphics::LineWidthLimits {
        min: 0.5,
        max: 4.0,
        granularity: 0.25,
    };
    let copied = limits;
    assert_eq!(limits, copied);
    assert_eq!(limits.min, 0.5);
    assert_eq!(limits.max, 4.0);
    assert_eq!(limits.granularity, 0.25);
    let error =
        vmnl_graphics::VMNLError::new(vmnl_graphics::VMNLErrorKind::DeviceFeatureNotEnabled {
            feature: vmnl_graphics::DeviceFeature::WideLines,
        });
    assert_eq!(
        error.to_string(),
        "device feature is not enabled: WideLines"
    );
    let error = vmnl_graphics::VMNLError::new(vmnl_graphics::VMNLErrorKind::InvalidLineWidth {
        value: 5.0,
        min: limits.min,
        max: limits.max,
    });
    assert_eq!(
        error.to_string(),
        "line width 5 must be finite, positive and within [0.5, 4]"
    );
}

#[test]
fn raw_pipeline_spec_exposes_face_culling() -> VMNLResult<()> {
    for cull_mode in [
        raw::CullMode::None,
        raw::CullMode::Front,
        raw::CullMode::Back,
        raw::CullMode::FrontAndBack,
    ] {
        for front_face in [raw::FrontFace::CounterClockwise, raw::FrontFace::Clockwise] {
            let spec = raw::Pipeline::<RawSpecVertex>::builder()
                .cull_mode(cull_mode)
                .front_face(front_face);
            assert_eq!(spec.cull_mode_value(), cull_mode);
            assert_eq!(spec.front_face_value(), front_face);
            assert_eq!(spec.topology_value(), raw::PrimitiveTopology::TriangleList);
            assert_eq!(spec.blend_mode_value(), raw::BlendMode::Opaque);
        }
    }

    Ok(())
}

#[test]
fn raw_shader_sources_store_inline_or_path_inputs() -> VMNLResult<()> {
    let inline = raw::ShaderSource::Src("#version 460\nvoid main() {}".to_string());
    let path = raw::ShaderSource::Path(PathBuf::from("shader.vert"));

    assert!(matches!(inline, raw::ShaderSource::Src(source) if source.contains("#version 460")));
    assert!(matches!(path, raw::ShaderSource::Path(path) if path.ends_with("shader.vert")));

    Ok(())
}

#[test]
fn raw_pipeline_spec_exposes_topology_and_blend_mode() -> VMNLResult<()> {
    let topologies = [
        raw::PrimitiveTopology::PointList,
        raw::PrimitiveTopology::LineList,
        raw::PrimitiveTopology::LineStrip,
        raw::PrimitiveTopology::TriangleList,
        raw::PrimitiveTopology::TriangleStrip,
    ];

    for topology in topologies {
        let spec = raw::Pipeline::<RawSpecVertex>::builder()
            .topology(topology)
            .blend_mode(raw::BlendMode::Alpha);
        assert_eq!(spec.topology_value(), topology);
        assert_eq!(spec.blend_mode_value(), raw::BlendMode::Alpha);
    }

    let spec = raw::Pipeline::<RawSpecVertex>::builder().blend_mode(raw::BlendMode::Opaque);
    assert_eq!(spec.topology_value(), raw::PrimitiveTopology::TriangleList);
    assert_eq!(spec.blend_mode_value(), raw::BlendMode::Opaque);

    Ok(())
}

#[test]
fn raw_pipeline_blend_presets_are_editable_and_keep_legacy_selection() {
    fn exhaustive_legacy_match(mode: raw::BlendMode) -> bool {
        match mode {
            raw::BlendMode::Opaque => false,
            raw::BlendMode::Alpha => true,
        }
    }

    let default = raw::Pipeline::<RawSpecVertex>::builder();
    assert_eq!(default.blend_mode_value(), raw::BlendMode::Opaque);
    assert_eq!(
        default.color_blend_config_value(),
        ColorBlendConfig::opaque()
    );
    assert!(!exhaustive_legacy_match(default.blend_mode_value()));

    let straight_alpha = ColorBlendConfig::straight_alpha();
    assert!(straight_alpha.blend_enabled());
    assert_eq!(
        straight_alpha.color_source_factor(),
        ColorBlendFactor::SourceAlpha
    );
    assert_eq!(
        straight_alpha.color_destination_factor(),
        ColorBlendFactor::OneMinusSourceAlpha
    );
    assert_eq!(straight_alpha.color_operation(), ColorBlendOperation::Add);
    assert_eq!(straight_alpha.alpha_source_factor(), ColorBlendFactor::One);
    assert_eq!(
        straight_alpha.alpha_destination_factor(),
        ColorBlendFactor::OneMinusSourceAlpha
    );
    assert_eq!(straight_alpha.alpha_operation(), ColorBlendOperation::Add);

    let premultiplied_alpha = ColorBlendConfig::premultiplied_alpha();
    assert_eq!(
        premultiplied_alpha.color_source_factor(),
        ColorBlendFactor::One
    );
    assert_eq!(
        premultiplied_alpha.color_destination_factor(),
        ColorBlendFactor::OneMinusSourceAlpha
    );
    assert_eq!(
        premultiplied_alpha.alpha_source_factor(),
        ColorBlendFactor::One
    );

    let customized = ColorBlendConfig::straight_alpha()
        .with_color_blend(
            ColorBlendFactor::SourceColor,
            ColorBlendFactor::OneMinusDestinationColor,
            ColorBlendOperation::ReverseSubtract,
        )
        .with_alpha_blend(
            ColorBlendFactor::ConstantAlpha,
            ColorBlendFactor::SourceAlphaSaturate,
            ColorBlendOperation::Max,
        )
        .with_blend_constants([0.1, 0.2, 0.3, 0.4]);
    assert_eq!(
        customized.color_source_factor(),
        ColorBlendFactor::SourceColor
    );
    assert_eq!(
        customized.color_destination_factor(),
        ColorBlendFactor::OneMinusDestinationColor
    );
    assert_eq!(
        customized.color_operation(),
        ColorBlendOperation::ReverseSubtract
    );
    assert_eq!(
        customized.alpha_source_factor(),
        ColorBlendFactor::ConstantAlpha
    );
    assert_eq!(
        customized.alpha_destination_factor(),
        ColorBlendFactor::SourceAlphaSaturate
    );
    assert_eq!(customized.alpha_operation(), ColorBlendOperation::Max);
    assert_eq!(customized.blend_constants(), [0.1, 0.2, 0.3, 0.4]);

    let explicit_after_legacy = raw::Pipeline::<RawSpecVertex>::builder()
        .blend_mode(raw::BlendMode::Alpha)
        .color_blend(customized);
    assert_eq!(
        explicit_after_legacy.blend_mode_value(),
        raw::BlendMode::Alpha
    );
    assert!(exhaustive_legacy_match(
        explicit_after_legacy.blend_mode_value()
    ));
    assert_eq!(explicit_after_legacy.color_blend_config_value(), customized);

    let legacy_after_explicit = raw::Pipeline::<RawSpecVertex>::builder()
        .color_blend(customized)
        .blend_mode(raw::BlendMode::Alpha);
    let legacy_alpha = ColorBlendConfig::straight_alpha().with_alpha_blend(
        ColorBlendFactor::SourceAlpha,
        ColorBlendFactor::OneMinusSourceAlpha,
        ColorBlendOperation::Add,
    );
    assert_eq!(
        legacy_after_explicit.blend_mode_value(),
        raw::BlendMode::Alpha
    );
    assert_eq!(
        legacy_after_explicit.color_blend_config_value(),
        legacy_alpha
    );
}

#[test]
fn public_graphics_defaults_are_stable() -> VMNLResult<()> {
    assert_eq!(PresentMode::default(), PresentMode::Auto);
    assert_eq!(RenderMode::default(), RenderMode::PerObject);
    assert_eq!(
        BufferMemoryPreference::default(),
        BufferMemoryPreference::Device
    );
    Ok(())
}
