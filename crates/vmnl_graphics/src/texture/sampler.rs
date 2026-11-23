// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Editable sampler requests and CPU-side numerical validation.

use crate::{VMNLError, VMNLErrorKind, VMNLResult};

/// Texture filtering mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// Select the nearest texel or mip level.
    Nearest,
    /// Interpolate between texels or mip levels.
    Linear,
}

/// Addressing policy for a sampler axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressMode {
    /// Clamp coordinates to the image edge.
    ClampToEdge,
    /// Repeat coordinates at integer boundaries.
    Repeat,
    /// Repeat coordinates while mirroring each alternate interval.
    MirroredRepeat,
    /// Use the configured border color outside the image.
    ClampToBorder,
}

/// Typed Vulkan border color request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SamplerBorderColor {
    /// Floating-point transparent black.
    #[default]
    FloatTransparentBlack,
    /// Integer transparent black.
    IntTransparentBlack,
    /// Floating-point opaque black.
    FloatOpaqueBlack,
    /// Integer opaque black.
    IntOpaqueBlack,
    /// Floating-point opaque white.
    FloatOpaqueWhite,
    /// Integer opaque white.
    IntOpaqueWhite,
}

/// Maximum permitted level of detail for a sampler request.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SamplerLodLimit {
    /// Leave the maximum level unbounded by the client request.
    #[default]
    Unbounded,
    /// Set a finite maximum level of detail.
    Value(f32),
}

/// Coordinate interpretation requested from a sampler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SamplerCoordinateMode {
    /// Use normalized coordinates in the range selected by the shader and sampler.
    #[default]
    Normalized,
    /// Use unnormalized texel coordinates with Vulkan's stricter sampler rules.
    /// Raw shader sampling must use explicit, statically known LOD zero and avoid
    /// implicit LOD, depth comparison, projection, bias, gradients, and offsets.
    Texel,
}

/// CPU-owned requested sampler settings, independent of GPU capability resolution.
///
/// The default and [`linear`](Self::linear) presets use linear filters, clamp-to-edge
/// addressing, zero minimum LOD and bias, unbounded maximum LOD, anisotropy `1.0`,
/// and normalized coordinates. [`nearest`](Self::nearest) changes all filters to
/// nearest. Values are requests; device limits are checked when the GPU sampler is built.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::texture::{AddressMode, Filter, SamplerBorderColor,
///     SamplerConfig, SamplerCoordinateMode, SamplerLodLimit};
///
/// assert_eq!(SamplerConfig::nearest().min_filter(), Filter::Nearest);
/// let config = SamplerConfig::linear()
///     .with_min_filter(Filter::Linear)
///     .with_mag_filter(Filter::Linear)
///     .with_mipmap_filter(Filter::Nearest)
///     .with_address_u(AddressMode::ClampToBorder)
///     .with_address_v(AddressMode::ClampToEdge)
///     .with_border_color(SamplerBorderColor::FloatOpaqueBlack)
///     .with_min_lod(0.0)
///     .with_max_lod(SamplerLodLimit::Value(1.0))
///     .with_lod_bias(0.0)
///     .with_anisotropy(1.0)
///     .with_coordinate_mode(SamplerCoordinateMode::Normalized);
/// config.validate()?;
/// assert_eq!(config.min_filter(), Filter::Linear);
/// assert_eq!(config.mag_filter(), Filter::Linear);
/// assert_eq!(config.mipmap_filter(), Filter::Nearest);
/// assert_eq!(config.address_u(), AddressMode::ClampToBorder);
/// assert_eq!(config.address_v(), AddressMode::ClampToEdge);
/// assert_eq!(config.border_color(), SamplerBorderColor::FloatOpaqueBlack);
/// assert_eq!(config.min_lod(), 0.0);
/// assert_eq!(config.max_lod(), SamplerLodLimit::Value(1.0));
/// assert_eq!(config.lod_bias(), 0.0);
/// assert_eq!(config.anisotropy(), 1.0);
/// assert_eq!(config.coordinate_mode(), SamplerCoordinateMode::Normalized);
/// # Ok::<(), vmnl_graphics::VMNLError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerConfig {
    min_filter: Filter,
    mag_filter: Filter,
    mipmap_filter: Filter,
    address_u: AddressMode,
    address_v: AddressMode,
    border_color: SamplerBorderColor,
    min_lod: f32,
    max_lod: SamplerLodLimit,
    lod_bias: f32,
    anisotropy: f32,
    coordinate_mode: SamplerCoordinateMode,
}

impl SamplerConfig {
    /// Returns the default linear configuration as an editable value.
    ///
    /// # Example
    ///
    /// See [`linear` in the shared example](Self#example).
    #[must_use]
    pub const fn linear() -> Self {
        Self {
            min_filter: Filter::Linear,
            mag_filter: Filter::Linear,
            mipmap_filter: Filter::Linear,
            address_u: AddressMode::ClampToEdge,
            address_v: AddressMode::ClampToEdge,
            border_color: SamplerBorderColor::FloatTransparentBlack,
            min_lod: 0.0,
            max_lod: SamplerLodLimit::Unbounded,
            lod_bias: 0.0,
            anisotropy: 1.0,
            coordinate_mode: SamplerCoordinateMode::Normalized,
        }
    }

    /// Returns an editable nearest-filter configuration.
    ///
    /// # Example
    ///
    /// See [`nearest` in the shared example](Self#example).
    #[must_use]
    pub const fn nearest() -> Self {
        Self {
            min_filter: Filter::Nearest,
            mag_filter: Filter::Nearest,
            mipmap_filter: Filter::Nearest,
            ..Self::linear()
        }
    }

    /// Sets minification filtering.
    ///
    /// # Arguments
    ///
    /// - `filter` selects the filter used when sampling a minified texture. See [`Filter`].
    ///
    /// # Example
    ///
    /// See [`with_min_filter` in the shared example](Self#example).
    #[must_use]
    pub const fn with_min_filter(mut self, filter: Filter) -> Self {
        self.min_filter = filter;
        self
    }

    /// Sets magnification filtering.
    ///
    /// # Arguments
    ///
    /// - `filter` selects the filter used when sampling a magnified texture. See [`Filter`].
    ///
    /// # Example
    ///
    /// See [`with_mag_filter` in the shared example](Self#example).
    #[must_use]
    pub const fn with_mag_filter(mut self, filter: Filter) -> Self {
        self.mag_filter = filter;
        self
    }

    /// Sets mipmap filtering.
    ///
    /// # Arguments
    ///
    /// - `filter` selects how adjacent mip levels are sampled. See [`Filter`].
    ///
    /// # Example
    ///
    /// See [`with_mipmap_filter` in the shared example](Self#example).
    #[must_use]
    pub const fn with_mipmap_filter(mut self, filter: Filter) -> Self {
        self.mipmap_filter = filter;
        self
    }

    /// Sets U-axis addressing.
    ///
    /// # Arguments
    ///
    /// - `mode` selects how out-of-range horizontal coordinates are addressed. See [`AddressMode`].
    ///
    /// # Example
    ///
    /// See [`with_address_u` in the shared example](Self#example).
    #[must_use]
    pub const fn with_address_u(mut self, mode: AddressMode) -> Self {
        self.address_u = mode;
        self
    }

    /// Sets V-axis addressing.
    ///
    /// # Arguments
    ///
    /// - `mode` selects how out-of-range vertical coordinates are addressed. See [`AddressMode`].
    ///
    /// # Example
    ///
    /// See [`with_address_v` in the shared example](Self#example).
    #[must_use]
    pub const fn with_address_v(mut self, mode: AddressMode) -> Self {
        self.address_v = mode;
        self
    }

    /// Sets the typed border color used by `ClampToBorder` addressing.
    ///
    /// # Arguments
    ///
    /// - `color` selects the typed border color for coordinates addressed via `ClampToBorder`. See
    ///   [`SamplerBorderColor`].
    ///
    /// # Example
    ///
    /// See [`with_border_color` in the shared example](Self#example).
    #[must_use]
    pub const fn with_border_color(mut self, color: SamplerBorderColor) -> Self {
        self.border_color = color;
        self
    }

    /// Sets the minimum level of detail.
    ///
    /// # Arguments
    ///
    /// - `value` supplies the requested lower bound in mip levels.
    ///
    /// # Example
    ///
    /// See [`with_min_lod` in the shared example](Self#example).
    #[must_use]
    pub const fn with_min_lod(mut self, value: f32) -> Self {
        self.min_lod = value;
        self
    }

    /// Sets the finite or unbounded maximum level of detail.
    ///
    /// # Arguments
    ///
    /// - `limit` supplies either an explicit upper bound in mip levels or the unbounded setting. See
    ///   [`SamplerLodLimit`].
    ///
    /// # Example
    ///
    /// See [`with_max_lod` in the shared example](Self#example).
    #[must_use]
    pub const fn with_max_lod(mut self, limit: SamplerLodLimit) -> Self {
        self.max_lod = limit;
        self
    }

    /// Sets the level-of-detail bias.
    ///
    /// # Arguments
    ///
    /// - `value` supplies the signed offset added to the computed level of detail.
    ///
    /// # Example
    ///
    /// See [`with_lod_bias` in the shared example](Self#example).
    #[must_use]
    pub const fn with_lod_bias(mut self, value: f32) -> Self {
        self.lod_bias = value;
        self
    }

    /// Sets anisotropy; `1.0` disables anisotropic filtering.
    ///
    /// # Arguments
    ///
    /// - `value` supplies the requested anisotropic filtering ratio, with `1.0` disabling it.
    ///
    /// # Example
    ///
    /// See [`with_anisotropy` in the shared example](Self#example).
    #[must_use]
    pub const fn with_anisotropy(mut self, value: f32) -> Self {
        self.anisotropy = value;
        self
    }

    /// Sets normalized or texel coordinate mode.
    ///
    /// # Arguments
    ///
    /// - `mode` selects a [`SamplerCoordinateMode`]. [`SamplerCoordinateMode::Normalized`] is
    ///   the default. [`SamplerCoordinateMode::Texel`] requires equal minification/magnification
    ///   filters, [`Filter::Nearest`] mip filtering, zero min/max LOD and LOD bias, anisotropy
    ///   `1.0`, and clamp-to-edge or clamp-to-border addressing on both axes.
    ///   [`Self::validate`] checks these combinations.
    ///
    /// # Example
    ///
    /// See [`with_coordinate_mode` in the shared example](Self#example).
    #[must_use]
    pub const fn with_coordinate_mode(mut self, mode: SamplerCoordinateMode) -> Self {
        self.coordinate_mode = mode;
        self
    }

    /// Returns minification filtering.
    ///
    /// # Example
    ///
    /// See [`min_filter` in the shared example](Self#example).
    #[must_use]
    pub const fn min_filter(self) -> Filter {
        self.min_filter
    }

    /// Returns magnification filtering.
    ///
    /// # Example
    ///
    /// See [`mag_filter` in the shared example](Self#example).
    #[must_use]
    pub const fn mag_filter(self) -> Filter {
        self.mag_filter
    }

    /// Returns mipmap filtering.
    ///
    /// # Example
    ///
    /// See [`mipmap_filter` in the shared example](Self#example).
    #[must_use]
    pub const fn mipmap_filter(self) -> Filter {
        self.mipmap_filter
    }

    /// Returns U-axis addressing.
    ///
    /// # Example
    ///
    /// See [`address_u` in the shared example](Self#example).
    #[must_use]
    pub const fn address_u(self) -> AddressMode {
        self.address_u
    }

    /// Returns V-axis addressing.
    ///
    /// # Example
    ///
    /// See [`address_v` in the shared example](Self#example).
    #[must_use]
    pub const fn address_v(self) -> AddressMode {
        self.address_v
    }

    /// Returns the typed border color request.
    ///
    /// # Example
    ///
    /// See [`border_color` in the shared example](Self#example).
    #[must_use]
    pub const fn border_color(self) -> SamplerBorderColor {
        self.border_color
    }

    /// Returns the minimum level of detail.
    ///
    /// # Example
    ///
    /// See [`min_lod` in the shared example](Self#example).
    #[must_use]
    pub const fn min_lod(self) -> f32 {
        self.min_lod
    }

    /// Returns the finite or unbounded maximum level of detail.
    ///
    /// # Example
    ///
    /// See [`max_lod` in the shared example](Self#example).
    #[must_use]
    pub const fn max_lod(self) -> SamplerLodLimit {
        self.max_lod
    }

    /// Returns the level-of-detail bias.
    ///
    /// # Example
    ///
    /// See [`lod_bias` in the shared example](Self#example).
    #[must_use]
    pub const fn lod_bias(self) -> f32 {
        self.lod_bias
    }

    /// Returns the requested anisotropy.
    ///
    /// # Example
    ///
    /// See [`anisotropy` in the shared example](Self#example).
    #[must_use]
    pub const fn anisotropy(self) -> f32 {
        self.anisotropy
    }

    /// Returns normalized or texel coordinate mode.
    ///
    /// # Example
    ///
    /// See [`coordinate_mode` in the shared example](Self#example).
    #[must_use]
    pub const fn coordinate_mode(self) -> SamplerCoordinateMode {
        self.coordinate_mode
    }

    /// Validates finite numeric values and coordinate-mode restrictions.
    ///
    /// Device limits, image/view compatibility, and shader sampling instructions
    /// require a `Context` and are checked when a sampler or consumer is built.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when a numeric value or texel-coordinate combination is invalid.
    ///
    /// # Example
    ///
    /// See [`validate` in the shared example](Self#example).
    pub fn validate(self) -> VMNLResult<()> {
        if !self.min_lod.is_finite() || !self.lod_bias.is_finite() {
            return Err(invalid_state("sampler LOD values must be finite"));
        }
        if let SamplerLodLimit::Value(max_lod) = self.max_lod {
            if !max_lod.is_finite() {
                return Err(invalid_state("finite sampler maximum LOD must be finite"));
            }
            if max_lod < self.min_lod {
                return Err(invalid_state(
                    "sampler maximum LOD must be at least its minimum LOD",
                ));
            }
        }
        if !self.anisotropy.is_finite() || self.anisotropy < 1.0 {
            return Err(invalid_state(
                "sampler anisotropy must be finite and at least 1.0",
            ));
        }

        if self.coordinate_mode == SamplerCoordinateMode::Texel {
            let lod_is_zero = is_zero(self.min_lod)
                && is_zero(self.lod_bias)
                && matches!(self.max_lod, SamplerLodLimit::Value(value) if is_zero(value));
            let address_is_unnormalized_compatible = [self.address_u, self.address_v]
                .into_iter()
                .all(|mode| matches!(mode, AddressMode::ClampToEdge | AddressMode::ClampToBorder));
            if self.min_filter != self.mag_filter
                || self.mipmap_filter != Filter::Nearest
                || !lod_is_zero
                || self.anisotropy.to_bits() != 1.0_f32.to_bits()
                || !address_is_unnormalized_compatible
            {
                return Err(invalid_state(
                    "texel coordinates require equal min/mag filters, nearest mip filtering, zero LOD values, disabled anisotropy, and clamp addressing",
                ));
            }
        }

        Ok(())
    }
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self::linear()
    }
}

fn is_zero(value: f32) -> bool {
    value.to_bits().trailing_zeros() >= 31
}

fn invalid_state(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::InvalidState(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_defaults_and_presets_preserve_requested_values() {
        assert_eq!(SamplerConfig::default(), SamplerConfig::linear());
        assert_eq!(SamplerConfig::default().min_filter(), Filter::Linear);
        assert_eq!(SamplerConfig::default().mag_filter(), Filter::Linear);
        assert_eq!(SamplerConfig::default().mipmap_filter(), Filter::Linear);
        assert_eq!(
            SamplerConfig::default().address_u(),
            AddressMode::ClampToEdge
        );
        assert_eq!(
            SamplerConfig::default().address_v(),
            AddressMode::ClampToEdge
        );
        assert_eq!(
            SamplerConfig::default().max_lod(),
            SamplerLodLimit::Unbounded
        );
        assert_eq!(
            SamplerConfig::default().anisotropy().to_bits(),
            1.0_f32.to_bits()
        );

        let nearest = SamplerConfig::nearest();
        assert_eq!(nearest.min_filter(), Filter::Nearest);
        assert_eq!(nearest.mag_filter(), Filter::Nearest);
        assert_eq!(nearest.mipmap_filter(), Filter::Nearest);

        let overridden = SamplerConfig::default()
            .with_min_filter(Filter::Nearest)
            .with_address_u(AddressMode::Repeat);
        assert_eq!(overridden.min_filter(), Filter::Nearest);
        assert_eq!(overridden.mag_filter(), Filter::Linear);
        assert_eq!(overridden.address_u(), AddressMode::Repeat);
        assert_eq!(overridden.address_v(), AddressMode::ClampToEdge);
    }

    #[test]
    fn sampler_validation_rejects_nonfinite_and_incompatible_exact_requests() {
        assert!(SamplerConfig::default()
            .with_anisotropy(f32::NAN)
            .validate()
            .is_err());
        assert!(SamplerConfig::default()
            .with_min_lod(2.0)
            .with_max_lod(SamplerLodLimit::Value(1.0))
            .validate()
            .is_err());

        let incompatible_texel =
            SamplerConfig::default().with_coordinate_mode(SamplerCoordinateMode::Texel);
        assert!(matches!(
            incompatible_texel.validate(),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message)
                if message.contains("texel coordinates require"))
        ));

        let compatible_texel = SamplerConfig::default()
            .with_min_filter(Filter::Nearest)
            .with_mag_filter(Filter::Nearest)
            .with_mipmap_filter(Filter::Nearest)
            .with_max_lod(SamplerLodLimit::Value(0.0))
            .with_coordinate_mode(SamplerCoordinateMode::Texel);
        assert!(compatible_texel.validate().is_ok());
    }
}
