// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Window surface color policy and its resolution against reported pairs.

use crate::{ColorFormat, VMNLError, VMNLErrorKind, VMNLResult};
use std::fmt;
use vulkano::swapchain::ColorSpace;

/// Presentation color space, independent of the swapchain image format.
///
/// Values reported by a window preserve their exact identity, including spaces without a named
/// constant in VMNL. Availability depends on the surface, device and enabled extensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceColorSpace(pub(crate) ColorSpace);

impl SurfaceColorSpace {
    /// The presentation engine interprets pixels as nonlinear sRGB.
    pub const SRGB_NON_LINEAR: Self = Self(ColorSpace::SrgbNonLinear);
}

impl fmt::Display for SurfaceColorSpace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}

/// Exact format and presentation color space of a surface capability or request.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::{ColorFormat, SurfaceColorPair, SurfaceColorSpace};
///
/// let pair = SurfaceColorPair::new(ColorFormat::B8G8R8A8_SRGB, SurfaceColorSpace::SRGB_NON_LINEAR);
/// assert_eq!(pair.format(), ColorFormat::B8G8R8A8_SRGB);
/// assert_eq!(pair.color_space(), SurfaceColorSpace::SRGB_NON_LINEAR);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceColorPair {
    format: ColorFormat,
    color_space: SurfaceColorSpace,
}

impl SurfaceColorPair {
    /// Forms an exact pair without querying a surface or checking support.
    ///
    /// # Example
    ///
    /// See [the shared example](Self#example).
    #[must_use]
    pub const fn new(format: ColorFormat, color_space: SurfaceColorSpace) -> Self {
        Self {
            format,
            color_space,
        }
    }

    /// Returns the swapchain attachment format in the pair.
    ///
    /// # Example
    ///
    /// See [the shared example](Self#example).
    #[must_use]
    pub const fn format(self) -> ColorFormat {
        self.format
    }

    /// Returns the presentation color space in the pair.
    ///
    /// # Example
    ///
    /// See [the shared example](Self#example).
    #[must_use]
    pub const fn color_space(self) -> SurfaceColorSpace {
        self.color_space
    }
}

/// Policy used when no requested preference is available.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfaceColorFallback {
    /// Returns an error when no preference matches.
    #[default]
    Reject,
    /// Selects the first pair representable by VMNL in the order reported by the backend.
    FirstReported,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum SurfaceColorRequest {
    #[default]
    LegacyAuto,
    Exact(SurfaceColorPair),
    Prefer {
        pairs: Vec<SurfaceColorPair>,
        fallback: SurfaceColorFallback,
    },
}

/// Per-window swapchain color policy, resolved at window creation.
///
/// Defaults to [`Self::legacy_auto`]. Values own their preference list and perform no Vulkan or
/// window work until passed to [`crate::WindowBuilder::build`].
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::{ColorFormat, SurfaceColorConfig, SurfaceColorFallback, SurfaceColorPair,
///     SurfaceColorSpace, Window};
///
/// let space = SurfaceColorSpace::SRGB_NON_LINEAR;
/// let pairs = [SurfaceColorPair::new(ColorFormat::R8G8B8A8_SRGB, space)];
/// assert!(SurfaceColorConfig::legacy_auto().is_legacy_auto());
/// let config = SurfaceColorConfig::prefer(pairs)?;
/// assert_eq!(config.fallback(), Some(SurfaceColorFallback::Reject));
/// let builder = Window::builder().surface_color(config);
/// assert_eq!(builder.surface_color_value().preferences(), Some(pairs.as_slice()));
/// # Ok::<(), vmnl_graphics::VMNLError>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfaceColorConfig(SurfaceColorRequest);

impl SurfaceColorConfig {
    /// Prefers BGRA8 sRGB with nonlinear sRGB presentation, then the first reported VMNL pair.
    ///
    /// # Example
    ///
    /// See the [shared example](Self#example).
    #[must_use]
    pub const fn legacy_auto() -> Self {
        Self(SurfaceColorRequest::LegacyAuto)
    }

    /// Requests an exact pair. An unavailable pair causes window creation to fail.
    ///
    /// # Example
    ///
    /// See the [resolved window example](crate::Window#example-window-surface-color).
    #[must_use]
    pub const fn exact(format: ColorFormat, color_space: SurfaceColorSpace) -> Self {
        Self(SurfaceColorRequest::Exact(SurfaceColorPair::new(
            format,
            color_space,
        )))
    }

    /// Requests the first available pair in caller order, with no fallback.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for an empty list or a repeated pair.
    ///
    /// # Example
    ///
    /// See the [shared example](Self#example).
    pub fn prefer(pairs: impl IntoIterator<Item = SurfaceColorPair>) -> VMNLResult<Self> {
        Self::prefer_with_fallback(pairs, SurfaceColorFallback::Reject)
    }

    /// Requests ordered preferences with an explicit fallback.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for an empty list or a repeated pair.
    ///
    /// # Example
    ///
    /// See the [shared example](Self#example).
    pub fn prefer_with_fallback(
        pairs: impl IntoIterator<Item = SurfaceColorPair>,
        fallback: SurfaceColorFallback,
    ) -> VMNLResult<Self> {
        let mut preferences = Vec::new();
        for pair in pairs {
            if preferences.contains(&pair) {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                    "surface color preferences must not repeat a pair".into(),
                )));
            }
            preferences.push(pair);
        }
        if preferences.is_empty() {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "surface color preferences must include at least one pair".into(),
            )));
        }
        Ok(Self(SurfaceColorRequest::Prefer {
            pairs: preferences,
            fallback,
        }))
    }

    /// Returns whether this is the historical automatic policy.
    ///
    /// # Example
    ///
    /// See the [shared example](Self#example).
    #[must_use]
    pub const fn is_legacy_auto(&self) -> bool {
        matches!(self.0, SurfaceColorRequest::LegacyAuto)
    }

    /// Returns the requested pair for an exact policy.
    ///
    /// # Example
    ///
    /// See the [resolved window example](crate::Window#example-window-surface-color).
    #[must_use]
    pub const fn exact_pair(&self) -> Option<SurfaceColorPair> {
        match self.0 {
            SurfaceColorRequest::Exact(pair) => Some(pair),
            _ => None,
        }
    }

    /// Borrows the caller-ordered pairs for a preference policy.
    ///
    /// # Example
    ///
    /// See the [shared example](Self#example).
    #[must_use]
    pub fn preferences(&self) -> Option<&[SurfaceColorPair]> {
        match &self.0 {
            SurfaceColorRequest::Prefer { pairs, .. } => Some(pairs),
            _ => None,
        }
    }

    /// Returns the fallback of a preference policy.
    ///
    /// # Example
    ///
    /// See the [shared example](Self#example).
    #[must_use]
    pub const fn fallback(&self) -> Option<SurfaceColorFallback> {
        match self.0 {
            SurfaceColorRequest::Prefer { fallback, .. } => Some(fallback),
            _ => None,
        }
    }

    pub(crate) fn select(&self, reported: &[SurfaceColorPair]) -> VMNLResult<SurfaceColorInfo> {
        let first_reported = || {
            reported
                .first()
                .map(|&pair| (pair, SurfaceColorSelection::FirstReported))
        };
        let selected = match &self.0 {
            SurfaceColorRequest::LegacyAuto => {
                let preferred = SurfaceColorPair::new(
                    ColorFormat::B8G8R8A8_SRGB,
                    SurfaceColorSpace::SRGB_NON_LINEAR,
                );
                if reported.contains(&preferred) {
                    Some((preferred, SurfaceColorSelection::LegacyPreferred))
                } else {
                    first_reported()
                }
            }
            SurfaceColorRequest::Exact(pair) => reported
                .contains(pair)
                .then_some((*pair, SurfaceColorSelection::Exact)),
            SurfaceColorRequest::Prefer { pairs, fallback } => pairs
                .iter()
                .copied()
                .enumerate()
                .find(|(_, pair)| reported.contains(pair))
                .map(|(index, pair)| (pair, SurfaceColorSelection::Preference { index }))
                .or_else(|| match fallback {
                    SurfaceColorFallback::Reject => None,
                    SurfaceColorFallback::FirstReported => first_reported(),
                }),
        };
        let (pair, selection) = selected.ok_or_else(|| {
            VMNLError::new(VMNLErrorKind::SurfaceColorUnavailable {
                requested: self.clone(),
            })
        })?;
        Ok(SurfaceColorInfo {
            requested: self.clone(),
            pair,
            selection,
        })
    }
}

/// Reason a window selected its resolved surface color pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfaceColorSelection {
    /// The exact requested pair was available.
    Exact,
    /// A pair in the caller's preference list was available.
    Preference {
        /// Zero-based index in the caller's list.
        index: usize,
    },
    /// The historical BGRA8 sRGB preference was available.
    LegacyPreferred,
    /// The policy fell back to the first reported pair.
    FirstReported,
}

/// Immutable snapshot of a window's creation request and resolved surface color pair.
///
/// Borrow it with [`crate::Window::surface_color_info`]. Its getters perform no backend queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceColorInfo {
    requested: SurfaceColorConfig,
    pair: SurfaceColorPair,
    selection: SurfaceColorSelection,
}

impl SurfaceColorInfo {
    /// Borrows the original window policy.
    ///
    /// # Example
    ///
    /// See the [resolved window example](crate::Window#example-window-surface-color).
    #[must_use]
    pub const fn requested(&self) -> &SurfaceColorConfig {
        &self.requested
    }

    /// Returns the format and color space selected at creation.
    ///
    /// # Example
    ///
    /// See the [resolved window example](crate::Window#example-window-surface-color).
    #[must_use]
    pub const fn pair(&self) -> SurfaceColorPair {
        self.pair
    }

    /// Returns how the pair was selected, including the caller preference index when applicable.
    ///
    /// # Example
    ///
    /// See the [resolved window example](crate::Window#example-window-surface-color).
    #[must_use]
    pub const fn selection(&self) -> SurfaceColorSelection {
        self.selection
    }

    /// Returns whether the policy fell back to the first reported pair.
    ///
    /// # Example
    ///
    /// See the [resolved window example](crate::Window#example-window-surface-color).
    #[must_use]
    pub const fn used_fallback(&self) -> bool {
        matches!(self.selection, SurfaceColorSelection::FirstReported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: SurfaceColorPair = SurfaceColorPair::new(
        ColorFormat::B8G8R8A8_SRGB,
        SurfaceColorSpace::SRGB_NON_LINEAR,
    );
    const OTHER_SPACE: SurfaceColorPair = SurfaceColorPair::new(
        ColorFormat::B8G8R8A8_SRGB,
        SurfaceColorSpace(ColorSpace::DisplayP3NonLinear),
    );
    const OTHER_FORMAT: SurfaceColorPair = SurfaceColorPair::new(
        ColorFormat::R8G8B8A8_SRGB,
        SurfaceColorSpace::SRGB_NON_LINEAR,
    );

    #[test]
    fn exact_matches_both_components_and_keeps_the_request() -> VMNLResult<()> {
        let config = SurfaceColorConfig::exact(LEGACY.format(), LEGACY.color_space());
        assert!(matches!(
            config.select(&[OTHER_SPACE, OTHER_FORMAT]),
            Err(error) if matches!(error.kind(), VMNLErrorKind::SurfaceColorUnavailable { requested } if *requested == config)
        ));
        let selected = config.select(&[OTHER_SPACE, LEGACY])?;
        assert_eq!(selected.pair(), LEGACY);
        assert_eq!(selected.requested(), &config);
        assert_eq!(selected.selection(), SurfaceColorSelection::Exact);
        assert!(!selected.used_fallback());
        Ok(())
    }

    #[test]
    fn preferences_follow_caller_order_and_expose_the_selected_index() -> VMNLResult<()> {
        let config = SurfaceColorConfig::prefer([OTHER_SPACE, OTHER_FORMAT, LEGACY])?;
        let selected = config.select(&[LEGACY, OTHER_FORMAT])?;
        assert_eq!(selected.pair(), OTHER_FORMAT);
        assert_eq!(
            selected.selection(),
            SurfaceColorSelection::Preference { index: 1 }
        );
        assert!(!selected.used_fallback());
        assert!(config.select(&[]).is_err());
        assert!(config
            .select(&[SurfaceColorPair::new(
                ColorFormat::B8G8R8A8_UNORM,
                SurfaceColorSpace::SRGB_NON_LINEAR,
            )])
            .is_err());
        Ok(())
    }

    #[test]
    fn legacy_and_explicit_fallback_keep_report_order() -> VMNLResult<()> {
        let legacy = SurfaceColorConfig::default();
        let selected = legacy.select(&[OTHER_SPACE, LEGACY])?;
        assert_eq!(selected.pair(), LEGACY);
        assert_eq!(selected.selection(), SurfaceColorSelection::LegacyPreferred);
        assert!(!selected.used_fallback());
        let preference = SurfaceColorConfig::prefer_with_fallback(
            [LEGACY],
            SurfaceColorFallback::FirstReported,
        )?;
        for config in [legacy, preference] {
            let selected = config.select(&[OTHER_SPACE, OTHER_FORMAT])?;
            assert_eq!(selected.pair(), OTHER_SPACE);
            assert_eq!(selected.selection(), SurfaceColorSelection::FirstReported);
            assert!(selected.used_fallback());
            assert!(config.select(&[]).is_err());
        }
        Ok(())
    }
}
