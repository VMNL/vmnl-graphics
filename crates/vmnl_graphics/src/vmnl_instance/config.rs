// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! CPU-side context configuration and device feature requirements.

use super::{Context, VMNLInstance};
use crate::{common::ColorBlendOverrides, texture::TextureDefaults, VMNLResult};
use std::rc::Rc;
use vulkano::device::DeviceFeatures;

/// Line-width capabilities of the selected physical device, in framebuffer units.
///
/// These limits describe hardware support, not activation of [`DeviceFeature::WideLines`].
/// Widths between supported increments may be rounded by the driver; the exact
/// pixel coverage also depends on the device's line rasterization rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineWidthLimits {
    /// Reported minimum line width (inclusive); may be zero. VMNL requires positive requests.
    pub min: f32,
    /// Maximum supported line width (inclusive).
    pub max: f32,
    /// Reported increment between supported widths. Additional widths may be supported.
    pub granularity: f32,
}

impl LineWidthLimits {
    pub(crate) fn from_physical_device(device: &vulkano::device::physical::PhysicalDevice) -> Self {
        let properties = device.properties();
        Self {
            min: properties.line_width_range[0],
            max: properties.line_width_range[1],
            granularity: properties.line_width_granularity,
        }
    }
}

/// Optional Vulkan device functionality selectable through VMNL.
///
/// Requiring a feature filters physical-device candidates and enables it on the
/// selected logical device. Capability support alone does not enable a feature.
/// Device limits still constrain the sizes and modes used by shaders/pipelines.
/// This catalogue is non-exhaustive so future features can be added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceFeature {
    /// Enable non-solid polygon fill modes, including wireframe and points.
    FillModeNonSolid,
    /// Enable line widths other than `1.0`, within the device's supported range.
    WideLines,
    /// Enable point sizes greater than `1.0`, within the device's supported range.
    LargePoints,
    /// Enable anisotropic sampler filtering.
    SamplerAnisotropy,
    /// Enable nonzero sampler LOD bias on portability-subset devices.
    SamplerMipLodBias,
    /// Enable non-identity image-view component swizzles on portability-subset devices.
    ImageViewFormatSwizzle,
    /// Enable compatible image-view format reinterpretation on portability-subset devices.
    ImageViewFormatReinterpretation,
}

impl DeviceFeature {
    pub(super) fn is_set_in(self, features: &DeviceFeatures) -> bool {
        match self {
            Self::FillModeNonSolid => features.fill_mode_non_solid,
            Self::WideLines => features.wide_lines,
            Self::LargePoints => features.large_points,
            Self::SamplerAnisotropy => features.sampler_anisotropy,
            Self::SamplerMipLodBias => features.sampler_mip_lod_bias,
            Self::ImageViewFormatSwizzle => features.image_view_format_swizzle,
            Self::ImageViewFormatReinterpretation => features.image_view_format_reinterpretation,
        }
    }
}

/// CPU-side requirements for the logical device created by a [`ContextBuilder`].
///
/// The default requests no optional features. Requirements accumulate without
/// duplicates, in first-request order. No GPU or GLFW initialization occurs
/// while configuring this value; its requirement list may allocate CPU memory.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::{DeviceConfig, DeviceFeature};
///
/// let device = DeviceConfig::default()
///     .require_feature(DeviceFeature::WideLines)
///     .require_features([DeviceFeature::WideLines, DeviceFeature::FillModeNonSolid]);
/// assert_eq!(
///     device.required_features(),
///     &[DeviceFeature::WideLines, DeviceFeature::FillModeNonSolid],
/// );
/// ```
#[derive(Clone, Debug, Default)]
pub struct DeviceConfig {
    required_features: Vec<DeviceFeature>,
}

impl DeviceConfig {
    /// Adds a required feature. Repeated requests have no effect.
    ///
    /// # Arguments
    ///
    /// - `feature` identifies a device feature that the selected GPU must support and enable. See
    ///   [`DeviceFeature`].
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub fn require_feature(mut self, feature: DeviceFeature) -> Self {
        if !self.required_features.contains(&feature) {
            self.required_features.push(feature);
        }

        self
    }

    /// Adds required features without replacing existing requirements.
    ///
    /// # Arguments
    ///
    /// - `features` supplies device features that must all be supported by the selected GPU. See
    ///   [`DeviceFeature`].
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub fn require_features(mut self, features: impl IntoIterator<Item = DeviceFeature>) -> Self {
        for feature in features {
            self = self.require_feature(feature);
        }

        self
    }

    /// Returns the required features, in first-request order without duplicates.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub fn required_features(&self) -> &[DeviceFeature] {
        &self.required_features
    }

    /// Translates the recorded VMNL device requests into the features passed to logical device
    /// creation.
    pub(super) fn enabled_features(&self) -> DeviceFeatures {
        DeviceFeatures {
            fill_mode_non_solid: self
                .required_features
                .contains(&DeviceFeature::FillModeNonSolid),
            wide_lines: self.required_features.contains(&DeviceFeature::WideLines),
            large_points: self.required_features.contains(&DeviceFeature::LargePoints),
            sampler_anisotropy: self
                .required_features
                .contains(&DeviceFeature::SamplerAnisotropy),
            sampler_mip_lod_bias: self
                .required_features
                .contains(&DeviceFeature::SamplerMipLodBias),
            image_view_format_swizzle: self
                .required_features
                .contains(&DeviceFeature::ImageViewFormatSwizzle),
            image_view_format_reinterpretation: self
                .required_features
                .contains(&DeviceFeature::ImageViewFormatReinterpretation),
            ..DeviceFeatures::empty()
        }
    }
}

/// Builder for a graphics [`Context`], with automatic device and queue selection.
///
/// Construction and configuration are CPU-only. The default preserves
/// [`Context::new`]'s automatic selection policy and requests no optional device
/// features. No Vulkan objects or GLFW lifetime are acquired until [`Self::build`].
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::{Context, DeviceConfig, DeviceFeature};
/// use vmnl_graphics::common::ColorBlendOverrides;
/// use vmnl_graphics::texture::TextureDefaults;
///
/// let device = DeviceConfig::default().require_feature(DeviceFeature::WideLines);
/// let defaults = TextureDefaults::default().with_mutable_format_views(Some(false));
/// let builder = Context::builder()
///     .device(device)
///     .texture_defaults(defaults)
///     .raw_color_blend(ColorBlendOverrides::default().with_blend_enabled(false));
/// assert_eq!(builder.device_config().required_features(), &[DeviceFeature::WideLines]);
/// assert_eq!(builder.texture_defaults_config(), defaults);
/// assert_eq!(builder.raw_color_blend_defaults().blend_enabled(), Some(false));
/// ```
#[derive(Clone, Debug, Default)]
pub struct ContextBuilder {
    device: DeviceConfig,
    textures: TextureDefaults,
    raw_color_blend: ColorBlendOverrides,
}

impl ContextBuilder {
    /// Replaces the device configuration, including all feature requirements.
    ///
    /// # Arguments
    ///
    /// - `config` supplies the required device features, typically from `DeviceConfig::default`. See
    ///   [`DeviceConfig`].
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub fn device(mut self, config: DeviceConfig) -> Self {
        self.device = config;
        self
    }

    /// Returns the current CPU-side device configuration.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn device_config(&self) -> &DeviceConfig {
        &self.device
    }

    /// Sets exact context defaults for texture creation and explicit views.
    ///
    /// # Arguments
    ///
    /// - `defaults` supplies the initial defaults for texture creation and explicit views. See
    ///   [`TextureDefaults`].
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn texture_defaults(mut self, defaults: TextureDefaults) -> Self {
        self.textures = defaults;
        self
    }

    /// Returns the texture defaults requested for this context.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn texture_defaults_config(&self) -> TextureDefaults {
        self.textures
    }

    /// Sets per-field defaults for new raw pipelines. Unset fields use VMNL's opaque preset.
    ///
    /// # Arguments
    ///
    /// - `defaults` supplies raw blend fields that initialize the context's pipeline defaults.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn raw_color_blend(mut self, defaults: ColorBlendOverrides) -> Self {
        self.raw_color_blend = defaults;
        self
    }

    /// Returns the blend fields explicitly configured as context defaults.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn raw_color_blend_defaults(&self) -> ColorBlendOverrides {
        self.raw_color_blend
    }

    /// Builds the complete context, enabling every requested device feature.
    ///
    /// This initializes GLFW and a Vulkan instance, filters physical devices by
    /// the baseline extensions/graphics queue and all requested features, then
    /// creates the selected logical device, queue and allocators. Existing
    /// device-type ranking and backend-dependent tie ordering are preserved.
    /// Requested features are never silently dropped. Temporary initialization
    /// objects are released on failure; clones of the result share one device.
    ///
    /// # Errors
    ///
    /// Returns `DeviceRequirementsNotMet` if no compatible GPU satisfies a
    /// nonempty feature request. An empty request preserves the existing
    /// `VulkanUnsupportedFeature` category when no compatible GPU exists.
    /// GLFW, instance enumeration/creation and device-creation failures retain
    /// their existing categories. Capability checks require the Vulkan instance;
    /// failure can occur after instance creation but before device creation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, DeviceConfig, DeviceFeature};
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::builder()
    ///     .device(DeviceConfig::default().require_feature(DeviceFeature::WideLines))
    ///     .build()?;
    /// assert!(context.is_device_feature_enabled(DeviceFeature::WideLines));
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> VMNLResult<Context> {
        Ok(Context {
            inner: Rc::new(VMNLInstance::new(
                &self.device,
                self.textures,
                self.raw_color_blend,
            )?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_requests_map_exactly_to_vulkan_flags() {
        let features = [
            DeviceFeature::FillModeNonSolid,
            DeviceFeature::WideLines,
            DeviceFeature::LargePoints,
            DeviceFeature::SamplerAnisotropy,
            DeviceFeature::SamplerMipLodBias,
            DeviceFeature::ImageViewFormatSwizzle,
            DeviceFeature::ImageViewFormatReinterpretation,
        ];
        for mask in 0..128 {
            let config = features.iter().enumerate().fold(
                DeviceConfig::default(),
                |config, (index, &feature)| {
                    if mask & (1 << index) != 0 {
                        config.require_feature(feature)
                    } else {
                        config
                    }
                },
            );
            let enabled = config.enabled_features();
            assert_eq!(enabled.fill_mode_non_solid, mask & 1 != 0);
            assert_eq!(enabled.wide_lines, mask & 2 != 0);
            assert_eq!(enabled.large_points, mask & 4 != 0);
            assert_eq!(enabled.sampler_anisotropy, mask & 8 != 0);
            assert_eq!(enabled.sampler_mip_lod_bias, mask & 16 != 0);
            assert_eq!(enabled.image_view_format_swizzle, mask & 32 != 0);
            assert_eq!(enabled.image_view_format_reinterpretation, mask & 64 != 0);
            assert!(!enabled.depth_clamp);
            assert!(!enabled.geometry_shader);
            for (index, &feature) in features.iter().enumerate() {
                assert_eq!(feature.is_set_in(&enabled), mask & (1 << index) != 0);
            }
        }
    }
}
