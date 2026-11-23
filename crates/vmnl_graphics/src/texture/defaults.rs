// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Context defaults for texture image creation and explicit views.

use super::{TextureComponentMapping, TextureFormat, TextureViewInterpretation};

/// Optional texture policies inherited by builders using a context.
///
/// Unset fields preserve the standard defaults. These settings are exact and
/// are validated when preparing an image or building an explicit view. This
/// value holds CPU configuration and owns no texture, view, or sampler.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::texture::{AlphaMode, ColorEncoding, TextureComponentMapping,
///     TextureDefaults, TextureFormat, TextureViewInterpretation};
///
/// let mapping = TextureComponentMapping::identity();
/// let interpretation = TextureViewInterpretation::new(ColorEncoding::Srgb, AlphaMode::Straight);
/// let defaults = TextureDefaults::default()
///     .with_mutable_format_views(Some(true))
///     .with_view_format(Some(TextureFormat::Rgba8Srgb))
///     .with_view_component_mapping(Some(mapping))
///     .with_view_interpretation(Some(interpretation));
/// assert_eq!(defaults.mutable_format_views(), Some(true));
/// assert_eq!(defaults.view_format(), Some(TextureFormat::Rgba8Srgb));
/// assert_eq!(defaults.view_component_mapping(), Some(mapping));
/// assert_eq!(defaults.view_interpretation(), Some(interpretation));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureDefaults {
    mutable_format_views: Option<bool>,
    view_format: Option<TextureFormat>,
    view_component_mapping: Option<TextureComponentMapping>,
    view_interpretation: Option<TextureViewInterpretation>,
}

impl TextureDefaults {
    /// Sets the context default for mutable-format image creation.
    ///
    /// # Arguments
    ///
    /// - `value` sets the default for texture builders that omit their own request. `Some(true)`
    ///   enables mutable-format views by default, `Some(false)` disables them, and `None` clears
    ///   this context default.
    ///
    /// # Example
    ///
    /// See [`with_mutable_format_views` in the shared example](Self#example).
    #[must_use]
    pub const fn with_mutable_format_views(mut self, value: Option<bool>) -> Self {
        self.mutable_format_views = value;
        self
    }

    /// Returns the context request for mutable-format image creation, if set.
    ///
    /// # Example
    ///
    /// See [`mutable_format_views` in the shared example](Self#example).
    #[must_use]
    pub const fn mutable_format_views(self) -> Option<bool> {
        self.mutable_format_views
    }

    /// Sets the context default format for explicitly built texture views.
    ///
    /// # Arguments
    ///
    /// - `value` supplies a [`TextureFormat`] for explicit views that omit their own format.
    ///   `None` clears this context default, allowing such views to inherit the texture's format.
    ///
    /// # Example
    ///
    /// See [`with_view_format` in the shared example](Self#example).
    #[must_use]
    pub const fn with_view_format(mut self, value: Option<TextureFormat>) -> Self {
        self.view_format = value;
        self
    }

    /// Returns the context request for the format for explicit views, if set.
    ///
    /// # Example
    ///
    /// See [`view_format` in the shared example](Self#example).
    #[must_use]
    pub const fn view_format(self) -> Option<TextureFormat> {
        self.view_format
    }

    /// Sets the component mapping for explicit views. Pass `None` to remove this context default.
    ///
    /// # Arguments
    ///
    /// - `value` supplies the component mapping used when an explicit view leaves the mapping
    ///   unspecified. See [`TextureComponentMapping`].
    ///
    /// # Example
    ///
    /// See [`with_view_component_mapping` in the shared example](Self#example).
    #[must_use]
    pub const fn with_view_component_mapping(
        mut self,
        value: Option<TextureComponentMapping>,
    ) -> Self {
        self.view_component_mapping = value;
        self
    }

    /// Returns the context request for the component mapping for explicit views, if set.
    ///
    /// # Example
    ///
    /// See [`view_component_mapping` in the shared example](Self#example).
    #[must_use]
    pub const fn view_component_mapping(self) -> Option<TextureComponentMapping> {
        self.view_component_mapping
    }

    /// Sets the sampled interpretation for explicit views. Pass `None` to remove this context default.
    ///
    /// # Arguments
    ///
    /// - `value` supplies the interpretation used when an explicit view leaves it unspecified. See
    ///   [`TextureViewInterpretation`].
    ///
    /// # Example
    ///
    /// See [`with_view_interpretation` in the shared example](Self#example).
    #[must_use]
    pub const fn with_view_interpretation(
        mut self,
        value: Option<TextureViewInterpretation>,
    ) -> Self {
        self.view_interpretation = value;
        self
    }

    /// Returns the context request for the sampled interpretation for explicit views, if set.
    ///
    /// # Example
    ///
    /// See [`view_interpretation` in the shared example](Self#example).
    #[must_use]
    pub const fn view_interpretation(self) -> Option<TextureViewInterpretation> {
        self.view_interpretation
    }
}
