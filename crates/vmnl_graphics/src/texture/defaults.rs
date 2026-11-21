// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Context defaults for texture image creation and explicit views.

use super::{TextureComponentMapping, TextureFormat, TextureViewInterpretation};

/// Optional texture policies inherited by builders using a context.
///
/// Unset fields preserve the standard defaults. These settings are exact and
/// are validated when preparing an image or building an explicit view. This
/// value holds CPU configuration and owns no texture, view, or sampler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureDefaults {
    mutable_format_views: Option<bool>,
    view_format: Option<TextureFormat>,
    view_component_mapping: Option<TextureComponentMapping>,
    view_interpretation: Option<TextureViewInterpretation>,
}

impl TextureDefaults {
    /// Sets mutable-format image creation. Pass `None` to remove this context default.
    #[must_use]
    pub const fn with_mutable_format_views(mut self, value: Option<bool>) -> Self {
        self.mutable_format_views = value;
        self
    }

    /// Returns the context request for mutable-format image creation, if set.
    #[must_use]
    pub const fn mutable_format_views(self) -> Option<bool> {
        self.mutable_format_views
    }

    /// Sets the format for explicit views. Pass `None` to remove this context default.
    #[must_use]
    pub const fn with_view_format(mut self, value: Option<TextureFormat>) -> Self {
        self.view_format = value;
        self
    }

    /// Returns the context request for the format for explicit views, if set.
    #[must_use]
    pub const fn view_format(self) -> Option<TextureFormat> {
        self.view_format
    }

    /// Sets the component mapping for explicit views. Pass `None` to remove this context default.
    #[must_use]
    pub const fn with_view_component_mapping(
        mut self,
        value: Option<TextureComponentMapping>,
    ) -> Self {
        self.view_component_mapping = value;
        self
    }

    /// Returns the context request for the component mapping for explicit views, if set.
    #[must_use]
    pub const fn view_component_mapping(self) -> Option<TextureComponentMapping> {
        self.view_component_mapping
    }

    /// Sets the sampled interpretation for explicit views. Pass `None` to remove this context default.
    #[must_use]
    pub const fn with_view_interpretation(
        mut self,
        value: Option<TextureViewInterpretation>,
    ) -> Self {
        self.view_interpretation = value;
        self
    }

    /// Returns the context request for the sampled interpretation for explicit views, if set.
    #[must_use]
    pub const fn view_interpretation(self) -> Option<TextureViewInterpretation> {
        self.view_interpretation
    }
}
