// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

use vmnl_graphics::{
    texture::{
        AddressMode, AlphaMode, ColorEncoding, Filter, PixelLayout, PixelOrigin, PixelRect,
        PixelSource, PngDecodeOptions, PreparedTexture2D, Sampler, SamplerConfig,
        SamplerCoordinateMode, SamplerLodLimit, Texture2D, Texture2DBuilder, TextureComponent,
        TextureComponentMapping, TextureFormat, TextureRetirementPolicy, TextureUsage,
        TextureUsageRequest, TextureView2D, TextureView2DBuilder, TextureViewInterpretation,
    },
    Context, VMNLErrorKind, VMNLResult,
};

#[test]
fn pixel_source_prepares_declared_range_and_origin() -> VMNLResult<()> {
    let bytes = [
        99, 99, 99, 99, 1, 2, 3, 4, 5, 6, 7, 8, 250, 251, 252, 253, 9, 10, 11, 12, 13, 14, 15, 16,
    ];
    let source = PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &bytes)?
        .with_byte_range(0..24)
        .with_layout(PixelLayout::new(4, 12, PixelOrigin::TopLeft))
        .with_usage(TextureUsageRequest::Exact(
            TextureUsage::SAMPLED.union(TextureUsage::TRANSFER_DESTINATION),
        ));

    assert_eq!(source.extent(), [2, 2]);
    assert_eq!(source.format(), TextureFormat::Rgba8Unorm);
    assert_eq!(source.byte_range(), Some(0..24));
    assert_eq!(
        source.usage(),
        TextureUsageRequest::Exact(TextureUsage::SAMPLED.union(TextureUsage::TRANSFER_DESTINATION))
    );
    let prepared = source.prepare()?;

    assert_eq!(prepared.extent(), [2, 2]);
    assert_eq!(prepared.format(), TextureFormat::Rgba8Unorm);
    assert_eq!(prepared.encoding(), ColorEncoding::Linear);
    assert_eq!(prepared.alpha_mode(), AlphaMode::Straight);
    assert_eq!(
        prepared.bytes(),
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    Ok(())
}

#[test]
fn pixel_rect_is_nonempty_checked_and_top_left_origin() -> VMNLResult<()> {
    let rect = PixelRect::new(1, 2, 3, 4)?;
    assert_eq!(
        (rect.x(), rect.y(), rect.width(), rect.height()),
        (1, 2, 3, 4)
    );
    assert!(rect.validate_within(4, 6).is_ok());
    assert!(rect.validate_within(3, 6).is_err());
    Ok(())
}

#[test]
fn sampler_config_preserves_independent_requested_values() -> VMNLResult<()> {
    let config = SamplerConfig::default()
        .with_min_filter(Filter::Nearest)
        .with_address_u(AddressMode::Repeat)
        .with_max_lod(SamplerLodLimit::Value(3.0));
    assert_eq!(config.min_filter(), Filter::Nearest);
    assert_eq!(config.mag_filter(), Filter::Linear);
    assert_eq!(config.address_u(), AddressMode::Repeat);
    assert_eq!(config.address_v(), AddressMode::ClampToEdge);
    assert_eq!(config.max_lod(), SamplerLodLimit::Value(3.0));
    assert_eq!(config.coordinate_mode(), SamplerCoordinateMode::Normalized);
    assert!(config.validate().is_ok());
    Ok(())
}

#[test]
fn png_decode_reports_malformed_source_category() {
    assert!(matches!(
        PngDecodeOptions::default().decode_png(&[1, 2, 3]),
        Err(error) if matches!(error.kind(), VMNLErrorKind::TextureDecodeFailed(_))
    ));
}

#[test]
fn texture_retirement_and_compatibility_controls_are_public() {
    assert_eq!(
        TextureRetirementPolicy::default(),
        TextureRetirementPolicy::Wait
    );
    assert_ne!(
        TextureRetirementPolicy::Wait,
        TextureRetirementPolicy::DeferToContext
    );

    let _collect: fn(&Context) -> VMNLResult<usize> = Context::collect_texture_retirements;
    let _validate_sampler: fn(&TextureView2D, &Sampler) -> VMNLResult<()> =
        TextureView2D::validate_sampler;
    let _view_builder: fn(&vmnl_graphics::texture::Texture2D) -> TextureView2DBuilder =
        TextureView2D::builder;
    let _mutable_view_request: fn(Texture2DBuilder<'static>, bool) -> Texture2DBuilder<'static> =
        Texture2DBuilder::with_mutable_format_views;
    let _mutable_view_request_inspection: fn(&Texture2DBuilder<'static>) -> bool =
        Texture2DBuilder::mutable_format_views_requested;
    let _mutable_view_enabled: fn(&Texture2D) -> bool = Texture2D::mutable_format_views_enabled;
    let _prepared_mutable_view_enabled: fn(&PreparedTexture2D) -> bool =
        PreparedTexture2D::mutable_format_views_enabled;
    let _supports_view_format: fn(&Texture2D, TextureFormat) -> bool =
        Texture2D::supports_view_format;
    let _view_format: fn(TextureView2DBuilder, TextureFormat) -> TextureView2DBuilder =
        TextureView2DBuilder::format;
    let _view_mapping: fn(TextureView2DBuilder, TextureComponentMapping) -> TextureView2DBuilder =
        TextureView2DBuilder::component_mapping;
    let _view_interpretation: fn(
        TextureView2DBuilder,
        TextureViewInterpretation,
    ) -> TextureView2DBuilder = TextureView2DBuilder::interpretation;
    let _view_build: fn(TextureView2DBuilder) -> VMNLResult<TextureView2D> =
        TextureView2DBuilder::build;
    let _requested_format: fn(&TextureView2D) -> Option<TextureFormat> =
        TextureView2D::requested_format;
    let _effective_format: fn(&TextureView2D) -> TextureFormat = TextureView2D::effective_format;
    let _requested_mapping: fn(&TextureView2D) -> Option<TextureComponentMapping> =
        TextureView2D::requested_component_mapping;
    let _effective_mapping: fn(&TextureView2D) -> TextureComponentMapping =
        TextureView2D::effective_component_mapping;
    let _requested_interpretation: fn(&TextureView2D) -> Option<TextureViewInterpretation> =
        TextureView2D::requested_interpretation;
    let _sampled_interpretation: fn(&TextureView2D) -> Option<TextureViewInterpretation> =
        TextureView2D::sampled_interpretation;
}

#[test]
fn texture_view_mapping_and_interpretation_are_editable_values() {
    let identity = TextureComponentMapping::identity();
    assert!(identity.is_identity());
    assert_eq!(identity.red(), TextureComponent::Red);
    assert_eq!(identity.green(), TextureComponent::Green);
    assert_eq!(identity.blue(), TextureComponent::Blue);
    assert_eq!(identity.alpha(), TextureComponent::Alpha);

    let red_blue = TextureComponentMapping::new(
        TextureComponent::Blue,
        TextureComponent::Green,
        TextureComponent::Red,
        TextureComponent::Alpha,
    );
    assert!(!red_blue.is_identity());
    assert_eq!(red_blue.red(), TextureComponent::Blue);
    assert_eq!(red_blue.blue(), TextureComponent::Red);
    let constants = TextureComponentMapping::new(
        TextureComponent::Zero,
        TextureComponent::One,
        TextureComponent::Red,
        TextureComponent::Alpha,
    );
    assert_eq!(constants.red(), TextureComponent::Zero);
    assert_eq!(constants.green(), TextureComponent::One);

    let interpretation = TextureViewInterpretation::new(ColorEncoding::Srgb, AlphaMode::Opaque);
    assert_eq!(interpretation.encoding(), ColorEncoding::Srgb);
    assert_eq!(interpretation.alpha_mode(), AlphaMode::Opaque);
}
