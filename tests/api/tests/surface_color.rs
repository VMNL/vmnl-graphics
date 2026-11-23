// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Headless window surface color configuration tests.

use vmnl_graphics::{
    ColorFormat, SurfaceColorConfig, SurfaceColorFallback, SurfaceColorPair, SurfaceColorSpace,
    VMNLErrorKind, VMNLResult, Window,
};

const PAIR: SurfaceColorPair = SurfaceColorPair::new(
    ColorFormat::B8G8R8A8_SRGB,
    SurfaceColorSpace::SRGB_NON_LINEAR,
);

#[test]
fn window_builder_exposes_each_surface_color_policy_without_a_context() -> VMNLResult<()> {
    let builder = Window::builder();
    assert!(builder.surface_color_value().is_legacy_auto());

    let exact = SurfaceColorConfig::exact(PAIR.format(), PAIR.color_space());
    let builder = builder.surface_color(exact);
    assert_eq!(builder.surface_color_value().exact_pair(), Some(PAIR));
    assert_eq!(builder.surface_color_value().fallback(), None);

    let preferences = SurfaceColorConfig::prefer([PAIR])?;
    assert_eq!(preferences.fallback(), Some(SurfaceColorFallback::Reject));
    assert_eq!(preferences.preferences(), Some([PAIR].as_slice()));
    let preferences =
        SurfaceColorConfig::prefer_with_fallback([PAIR], SurfaceColorFallback::FirstReported)?;
    let builder = builder.surface_color(preferences.clone());
    assert_eq!(builder.surface_color_value(), &preferences);
    Ok(())
}

#[test]
fn invalid_preference_lists_fail_during_cpu_configuration() {
    for result in [
        SurfaceColorConfig::prefer([]),
        SurfaceColorConfig::prefer([PAIR, PAIR]),
    ] {
        assert!(
            matches!(result, Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(_)))
        );
    }
}
