// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Headless contracts for context and device configuration.

use vmnl_graphics::{
    common::{ColorBlendConfig, ColorBlendFactor, ColorBlendOperation, ColorBlendOverrides},
    raw,
    texture::{
        AlphaMode, ColorEncoding, TextureDefaults, TextureFormat, TextureViewInterpretation,
    },
    Context, ContextBuilder, DeviceConfig, DeviceFeature, VMNLError, VMNLErrorKind,
};

#[test]
fn raw_blend_resolves_each_field_and_preserves_explicit_presets() {
    let defaults = ColorBlendOverrides::default()
        .with_blend_enabled(true)
        .with_color_source_factor(ColorBlendFactor::SourceAlpha)
        .with_color_destination_factor(ColorBlendFactor::OneMinusSourceAlpha)
        .with_alpha_operation(ColorBlendOperation::Subtract)
        .with_blend_constants([0.25, 0.5, 0.75, 1.0]);
    let builder = Context::builder().raw_color_blend(defaults);
    let base = builder
        .raw_color_blend_defaults()
        .resolve(ColorBlendConfig::opaque());
    let local = ColorBlendOverrides::default()
        .with_color_operation(ColorBlendOperation::Max)
        .with_alpha_source_factor(ColorBlendFactor::DestinationAlpha)
        .with_alpha_destination_factor(ColorBlendFactor::Zero);
    let spec = raw::Pipeline::<()>::builder().color_blend_overrides(local);
    assert_eq!(spec.color_blend_overrides_value(), local);
    assert_eq!(local.blend_enabled(), None);
    let resolved = spec.color_blend_overrides_value().resolve(base);
    assert!(resolved.blend_enabled());
    assert_eq!(
        resolved.color_source_factor(),
        ColorBlendFactor::SourceAlpha
    );
    assert_eq!(
        resolved.color_destination_factor(),
        ColorBlendFactor::OneMinusSourceAlpha
    );
    assert_eq!(resolved.color_operation(), ColorBlendOperation::Max);
    assert_eq!(resolved.alpha_operation(), ColorBlendOperation::Subtract);
    assert_eq!(
        resolved.alpha_source_factor(),
        ColorBlendFactor::DestinationAlpha
    );
    assert_eq!(resolved.alpha_destination_factor(), ColorBlendFactor::Zero);
    assert_eq!(resolved.blend_constants(), [0.25, 0.5, 0.75, 1.0]);
    let legacy = spec.clone().blend_mode(raw::BlendMode::Alpha);
    assert_eq!(
        legacy.color_blend_overrides_value().resolve(base),
        legacy.color_blend_config_value()
    );
    let explicit = legacy.color_blend(ColorBlendConfig::straight_alpha());
    assert_eq!(
        explicit.color_blend_overrides_value().resolve(base),
        ColorBlendConfig::straight_alpha()
    );
    assert_eq!(
        explicit
            .color_blend_overrides(local)
            .color_blend_overrides_value()
            .resolve(base),
        resolved
    );
    assert_eq!(
        Context::builder().raw_color_blend_defaults(),
        ColorBlendOverrides::default()
    );
}

#[test]
fn texture_context_defaults_keep_omitted_and_explicit_settings_distinct() {
    let interpretation = TextureViewInterpretation::new(ColorEncoding::Linear, AlphaMode::Opaque);
    let defaults = TextureDefaults::default()
        .with_mutable_format_views(Some(false))
        .with_view_format(Some(TextureFormat::Rgba8Srgb))
        .with_view_interpretation(Some(interpretation));
    let builder = Context::builder().texture_defaults(defaults);
    let request = builder.texture_defaults_config();
    assert_eq!(request, defaults);
    assert_eq!(request.mutable_format_views(), Some(false));
    assert_eq!(request.view_format(), Some(TextureFormat::Rgba8Srgb));
    assert_eq!(request.view_component_mapping(), None);
    assert_eq!(request.view_interpretation(), Some(interpretation));
    assert_eq!(
        request
            .with_mutable_format_views(None)
            .mutable_format_views(),
        None
    );
}

#[test]
fn context_builder_defaults_require_no_device_features() {
    assert!(Context::builder()
        .device_config()
        .required_features()
        .is_empty());
    assert!(ContextBuilder::default()
        .device_config()
        .required_features()
        .is_empty());
    assert!(DeviceConfig::default().required_features().is_empty());
}

#[test]
fn device_requirements_accumulate_without_duplicates() {
    let config = DeviceConfig::default()
        .require_feature(DeviceFeature::WideLines)
        .require_features([
            DeviceFeature::FillModeNonSolid,
            DeviceFeature::WideLines,
            DeviceFeature::LargePoints,
        ]);
    assert_eq!(
        config.required_features(),
        &[
            DeviceFeature::WideLines,
            DeviceFeature::FillModeNonSolid,
            DeviceFeature::LargePoints,
        ]
    );
}

#[test]
fn texture_features_are_explicit_device_requirements() {
    let features = [
        DeviceFeature::SamplerAnisotropy,
        DeviceFeature::SamplerMipLodBias,
        DeviceFeature::ImageViewFormatSwizzle,
        DeviceFeature::ImageViewFormatReinterpretation,
    ];
    let config = DeviceConfig::default().require_features(features);

    assert_eq!(config.required_features(), &features);
    assert!(Context::builder()
        .device_config()
        .required_features()
        .is_empty());
}

#[test]
fn context_device_configuration_is_replaced_and_can_be_reused() {
    let first = DeviceConfig::default().require_feature(DeviceFeature::FillModeNonSolid);
    let second = DeviceConfig::default().require_feature(DeviceFeature::LargePoints);
    let builder = Context::builder().device(first).device(second.clone());
    assert_eq!(
        builder.device_config().required_features(),
        &[DeviceFeature::LargePoints]
    );
    let reused = Context::builder().device(second);
    assert_eq!(
        reused.device_config().required_features(),
        builder.device_config().required_features()
    );
    assert_eq!(
        builder.clone().device_config().required_features(),
        &[DeviceFeature::LargePoints]
    );
}

#[test]
fn unmet_device_requirements_are_structured_and_displayed() {
    let error = VMNLError::new(VMNLErrorKind::DeviceRequirementsNotMet {
        required_features: vec![DeviceFeature::FillModeNonSolid, DeviceFeature::WideLines],
    });
    assert!(
        matches!(error.kind(), VMNLErrorKind::DeviceRequirementsNotMet { required_features }
        if required_features == &[DeviceFeature::FillModeNonSolid, DeviceFeature::WideLines])
    );
    assert_eq!(
        error.to_string(),
        "no compatible Vulkan device satisfies required features: [FillModeNonSolid, WideLines]"
    );
}
