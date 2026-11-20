// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Backend-independent color attachment blend configuration.

/// Factor used to weight a source or destination color during blending.
///
/// The `Source1*` factors are not exposed because VMNL does not currently let
/// clients request Vulkan's `dualSrcBlend` device feature.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorBlendFactor {
    /// Factor value `0`.
    Zero,
    /// Factor value `1`.
    One,
    /// Source color, component-wise.
    SourceColor,
    /// One minus the source color, component-wise.
    OneMinusSourceColor,
    /// Destination color, component-wise.
    DestinationColor,
    /// One minus the destination color, component-wise.
    OneMinusDestinationColor,
    /// Source alpha for every component.
    SourceAlpha,
    /// One minus source alpha for every component.
    OneMinusSourceAlpha,
    /// Destination alpha for every component.
    DestinationAlpha,
    /// One minus destination alpha for every component.
    OneMinusDestinationAlpha,
    /// RGBA blend constant, component-wise for RGB and its alpha for alpha output.
    ConstantColor,
    /// One minus the RGBA blend constant, component-wise.
    OneMinusConstantColor,
    /// Blend constant alpha for every component.
    ConstantAlpha,
    /// One minus blend constant alpha for every component.
    OneMinusConstantAlpha,
    /// `min(source alpha, 1 - destination alpha)` for RGB; `1` for alpha.
    SourceAlphaSaturate,
}

impl ColorBlendFactor {
    pub(crate) const fn to_vulkano(self) -> vulkano::pipeline::graphics::color_blend::BlendFactor {
        use vulkano::pipeline::graphics::color_blend::BlendFactor;

        match self {
            Self::Zero => BlendFactor::Zero,
            Self::One => BlendFactor::One,
            Self::SourceColor => BlendFactor::SrcColor,
            Self::OneMinusSourceColor => BlendFactor::OneMinusSrcColor,
            Self::DestinationColor => BlendFactor::DstColor,
            Self::OneMinusDestinationColor => BlendFactor::OneMinusDstColor,
            Self::SourceAlpha => BlendFactor::SrcAlpha,
            Self::OneMinusSourceAlpha => BlendFactor::OneMinusSrcAlpha,
            Self::DestinationAlpha => BlendFactor::DstAlpha,
            Self::OneMinusDestinationAlpha => BlendFactor::OneMinusDstAlpha,
            Self::ConstantColor => BlendFactor::ConstantColor,
            Self::OneMinusConstantColor => BlendFactor::OneMinusConstantColor,
            Self::ConstantAlpha => BlendFactor::ConstantAlpha,
            Self::OneMinusConstantAlpha => BlendFactor::OneMinusConstantAlpha,
            Self::SourceAlphaSaturate => BlendFactor::SrcAlphaSaturate,
        }
    }
}

/// Operation applied to weighted source and destination color components.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorBlendOperation {
    /// Add the weighted source and destination components.
    Add,
    /// Subtract the weighted destination from the weighted source.
    Subtract,
    /// Subtract the weighted source from the weighted destination.
    ReverseSubtract,
    /// Select the smaller source or destination component.
    Min,
    /// Select the larger source or destination component.
    Max,
}

impl ColorBlendOperation {
    pub(crate) const fn to_vulkano(self) -> vulkano::pipeline::graphics::color_blend::BlendOp {
        use vulkano::pipeline::graphics::color_blend::BlendOp;

        match self {
            Self::Add => BlendOp::Add,
            Self::Subtract => BlendOp::Subtract,
            Self::ReverseSubtract => BlendOp::ReverseSubtract,
            Self::Min => BlendOp::Min,
            Self::Max => BlendOp::Max,
        }
    }
}

/// Complete color and alpha blend state for a raw pipeline's color attachment.
///
/// The default disables blending. Presets are editable values; use the `with_*`
/// methods to replace any part before passing the configuration to
/// `raw::PipelineSpec::color_blend`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorBlendConfig {
    blend_enabled: bool,
    color_source_factor: ColorBlendFactor,
    color_destination_factor: ColorBlendFactor,
    color_operation: ColorBlendOperation,
    alpha_source_factor: ColorBlendFactor,
    alpha_destination_factor: ColorBlendFactor,
    alpha_operation: ColorBlendOperation,
    blend_constants: [f32; 4],
}

impl Default for ColorBlendConfig {
    fn default() -> Self {
        Self::opaque()
    }
}

impl ColorBlendConfig {
    /// Returns blending-disabled state, which writes shader output directly.
    #[must_use]
    pub const fn opaque() -> Self {
        Self {
            blend_enabled: false,
            color_source_factor: ColorBlendFactor::One,
            color_destination_factor: ColorBlendFactor::Zero,
            color_operation: ColorBlendOperation::Add,
            alpha_source_factor: ColorBlendFactor::One,
            alpha_destination_factor: ColorBlendFactor::Zero,
            alpha_operation: ColorBlendOperation::Add,
            blend_constants: [0.0; 4],
        }
    }

    /// Returns straight-alpha source-over blending.
    ///
    /// RGB is `Cs * As + Cd * (1 - As)`; alpha is `As + Ad * (1 - As)`.
    #[must_use]
    pub const fn straight_alpha() -> Self {
        Self {
            blend_enabled: true,
            color_source_factor: ColorBlendFactor::SourceAlpha,
            color_destination_factor: ColorBlendFactor::OneMinusSourceAlpha,
            color_operation: ColorBlendOperation::Add,
            alpha_source_factor: ColorBlendFactor::One,
            alpha_destination_factor: ColorBlendFactor::OneMinusSourceAlpha,
            alpha_operation: ColorBlendOperation::Add,
            blend_constants: [0.0; 4],
        }
    }

    /// Returns premultiplied-alpha source-over blending.
    ///
    /// RGB is `Cs + Cd * (1 - As)`; the shader must write premultiplied `Cs`.
    /// Alpha is `As + Ad * (1 - As)`.
    #[must_use]
    pub const fn premultiplied_alpha() -> Self {
        Self {
            blend_enabled: true,
            color_source_factor: ColorBlendFactor::One,
            color_destination_factor: ColorBlendFactor::OneMinusSourceAlpha,
            color_operation: ColorBlendOperation::Add,
            alpha_source_factor: ColorBlendFactor::One,
            alpha_destination_factor: ColorBlendFactor::OneMinusSourceAlpha,
            alpha_operation: ColorBlendOperation::Add,
            blend_constants: [0.0; 4],
        }
    }

    /// Enables or disables blending without changing the configured factors.
    #[must_use]
    pub const fn with_blend_enabled(mut self, enabled: bool) -> Self {
        self.blend_enabled = enabled;
        self
    }

    /// Replaces the independent RGB source/destination factors and operation.
    #[must_use]
    pub const fn with_color_blend(
        mut self,
        source: ColorBlendFactor,
        destination: ColorBlendFactor,
        operation: ColorBlendOperation,
    ) -> Self {
        self.color_source_factor = source;
        self.color_destination_factor = destination;
        self.color_operation = operation;
        self
    }

    /// Replaces the independent alpha source/destination factors and operation.
    #[must_use]
    pub const fn with_alpha_blend(
        mut self,
        source: ColorBlendFactor,
        destination: ColorBlendFactor,
        operation: ColorBlendOperation,
    ) -> Self {
        self.alpha_source_factor = source;
        self.alpha_destination_factor = destination;
        self.alpha_operation = operation;
        self
    }

    /// Sets the blend constant in RGBA order; values are passed to Vulkan unchanged.
    #[must_use]
    pub const fn with_blend_constants(mut self, rgba: [f32; 4]) -> Self {
        self.blend_constants = rgba;
        self
    }

    /// Returns whether blending is enabled.
    #[must_use]
    pub const fn blend_enabled(self) -> bool {
        self.blend_enabled
    }

    /// Returns the RGB source factor.
    #[must_use]
    pub const fn color_source_factor(self) -> ColorBlendFactor {
        self.color_source_factor
    }

    /// Returns the RGB destination factor.
    #[must_use]
    pub const fn color_destination_factor(self) -> ColorBlendFactor {
        self.color_destination_factor
    }

    /// Returns the RGB operation.
    #[must_use]
    pub const fn color_operation(self) -> ColorBlendOperation {
        self.color_operation
    }

    /// Returns the alpha source factor.
    #[must_use]
    pub const fn alpha_source_factor(self) -> ColorBlendFactor {
        self.alpha_source_factor
    }

    /// Returns the alpha destination factor.
    #[must_use]
    pub const fn alpha_destination_factor(self) -> ColorBlendFactor {
        self.alpha_destination_factor
    }

    /// Returns the alpha operation.
    #[must_use]
    pub const fn alpha_operation(self) -> ColorBlendOperation {
        self.alpha_operation
    }

    /// Returns the blend constant in RGBA order.
    #[must_use]
    pub const fn blend_constants(self) -> [f32; 4] {
        self.blend_constants
    }

    pub(crate) const fn legacy_alpha() -> Self {
        Self {
            blend_enabled: true,
            color_source_factor: ColorBlendFactor::SourceAlpha,
            color_destination_factor: ColorBlendFactor::OneMinusSourceAlpha,
            color_operation: ColorBlendOperation::Add,
            alpha_source_factor: ColorBlendFactor::SourceAlpha,
            alpha_destination_factor: ColorBlendFactor::OneMinusSourceAlpha,
            alpha_operation: ColorBlendOperation::Add,
            blend_constants: [0.0; 4],
        }
    }
}

/// Partial blend settings resolved against context defaults, then VMNL defaults.
///
/// Each setter selects an exact field value. An unset field inherits. A complete
/// [`ColorBlendConfig`] can be converted into an override for every field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorBlendOverrides {
    blend_enabled: Option<bool>,
    color_source_factor: Option<ColorBlendFactor>,
    color_destination_factor: Option<ColorBlendFactor>,
    color_operation: Option<ColorBlendOperation>,
    alpha_source_factor: Option<ColorBlendFactor>,
    alpha_destination_factor: Option<ColorBlendFactor>,
    alpha_operation: Option<ColorBlendOperation>,
    blend_constants: Option<[f32; 4]>,
}

impl ColorBlendOverrides {
    pub(crate) const EMPTY: Self = Self {
        blend_enabled: None,
        color_source_factor: None,
        color_destination_factor: None,
        color_operation: None,
        alpha_source_factor: None,
        alpha_destination_factor: None,
        alpha_operation: None,
        blend_constants: None,
    };

    /// Converts every value of a complete blend configuration into an explicit override.
    #[must_use]
    pub const fn from_config(config: ColorBlendConfig) -> Self {
        Self {
            blend_enabled: Some(config.blend_enabled),
            color_source_factor: Some(config.color_source_factor),
            color_destination_factor: Some(config.color_destination_factor),
            color_operation: Some(config.color_operation),
            alpha_source_factor: Some(config.alpha_source_factor),
            alpha_destination_factor: Some(config.alpha_destination_factor),
            alpha_operation: Some(config.alpha_operation),
            blend_constants: Some(config.blend_constants),
        }
    }

    /// Resolves the override against a complete base configuration without GPU work.
    #[must_use]
    pub const fn resolve(self, mut base: ColorBlendConfig) -> ColorBlendConfig {
        if let Some(value) = self.blend_enabled {
            base.blend_enabled = value;
        }
        if let Some(value) = self.color_source_factor {
            base.color_source_factor = value;
        }
        if let Some(value) = self.color_destination_factor {
            base.color_destination_factor = value;
        }
        if let Some(value) = self.color_operation {
            base.color_operation = value;
        }
        if let Some(value) = self.alpha_source_factor {
            base.alpha_source_factor = value;
        }
        if let Some(value) = self.alpha_destination_factor {
            base.alpha_destination_factor = value;
        }
        if let Some(value) = self.alpha_operation {
            base.alpha_operation = value;
        }
        if let Some(value) = self.blend_constants {
            base.blend_constants = value;
        }
        base
    }

    /// Sets whether blending is enabled and preserves all other overrides.
    #[must_use]
    pub const fn with_blend_enabled(mut self, value: bool) -> Self {
        self.blend_enabled = Some(value);
        self
    }

    /// Returns the explicit override for whether blending is enabled, if supplied.
    #[must_use]
    pub const fn blend_enabled(self) -> Option<bool> {
        self.blend_enabled
    }

    /// Sets the RGB source factor and preserves all other overrides.
    #[must_use]
    pub const fn with_color_source_factor(mut self, value: ColorBlendFactor) -> Self {
        self.color_source_factor = Some(value);
        self
    }

    /// Returns the explicit override for the RGB source factor, if supplied.
    #[must_use]
    pub const fn color_source_factor(self) -> Option<ColorBlendFactor> {
        self.color_source_factor
    }

    /// Sets the RGB destination factor and preserves all other overrides.
    #[must_use]
    pub const fn with_color_destination_factor(mut self, value: ColorBlendFactor) -> Self {
        self.color_destination_factor = Some(value);
        self
    }

    /// Returns the explicit override for the RGB destination factor, if supplied.
    #[must_use]
    pub const fn color_destination_factor(self) -> Option<ColorBlendFactor> {
        self.color_destination_factor
    }

    /// Sets the RGB operation and preserves all other overrides.
    #[must_use]
    pub const fn with_color_operation(mut self, value: ColorBlendOperation) -> Self {
        self.color_operation = Some(value);
        self
    }

    /// Returns the explicit override for the RGB operation, if supplied.
    #[must_use]
    pub const fn color_operation(self) -> Option<ColorBlendOperation> {
        self.color_operation
    }

    /// Sets the alpha source factor and preserves all other overrides.
    #[must_use]
    pub const fn with_alpha_source_factor(mut self, value: ColorBlendFactor) -> Self {
        self.alpha_source_factor = Some(value);
        self
    }

    /// Returns the explicit override for the alpha source factor, if supplied.
    #[must_use]
    pub const fn alpha_source_factor(self) -> Option<ColorBlendFactor> {
        self.alpha_source_factor
    }

    /// Sets the alpha destination factor and preserves all other overrides.
    #[must_use]
    pub const fn with_alpha_destination_factor(mut self, value: ColorBlendFactor) -> Self {
        self.alpha_destination_factor = Some(value);
        self
    }

    /// Returns the explicit override for the alpha destination factor, if supplied.
    #[must_use]
    pub const fn alpha_destination_factor(self) -> Option<ColorBlendFactor> {
        self.alpha_destination_factor
    }

    /// Sets the alpha operation and preserves all other overrides.
    #[must_use]
    pub const fn with_alpha_operation(mut self, value: ColorBlendOperation) -> Self {
        self.alpha_operation = Some(value);
        self
    }

    /// Returns the explicit override for the alpha operation, if supplied.
    #[must_use]
    pub const fn alpha_operation(self) -> Option<ColorBlendOperation> {
        self.alpha_operation
    }

    /// Sets the RGBA blend constants and preserves all other overrides.
    #[must_use]
    pub const fn with_blend_constants(mut self, value: [f32; 4]) -> Self {
        self.blend_constants = Some(value);
        self
    }

    /// Returns the explicit override for the RGBA blend constants, if supplied.
    #[must_use]
    pub const fn blend_constants(self) -> Option<[f32; 4]> {
        self.blend_constants
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vulkano::pipeline::graphics::color_blend::{BlendFactor, BlendOp};

    #[test]
    fn public_factors_map_to_vulkan_factors() {
        let factors = [
            (ColorBlendFactor::Zero, BlendFactor::Zero),
            (ColorBlendFactor::One, BlendFactor::One),
            (ColorBlendFactor::SourceColor, BlendFactor::SrcColor),
            (
                ColorBlendFactor::OneMinusSourceColor,
                BlendFactor::OneMinusSrcColor,
            ),
            (ColorBlendFactor::DestinationColor, BlendFactor::DstColor),
            (
                ColorBlendFactor::OneMinusDestinationColor,
                BlendFactor::OneMinusDstColor,
            ),
            (ColorBlendFactor::SourceAlpha, BlendFactor::SrcAlpha),
            (
                ColorBlendFactor::OneMinusSourceAlpha,
                BlendFactor::OneMinusSrcAlpha,
            ),
            (ColorBlendFactor::DestinationAlpha, BlendFactor::DstAlpha),
            (
                ColorBlendFactor::OneMinusDestinationAlpha,
                BlendFactor::OneMinusDstAlpha,
            ),
            (ColorBlendFactor::ConstantColor, BlendFactor::ConstantColor),
            (
                ColorBlendFactor::OneMinusConstantColor,
                BlendFactor::OneMinusConstantColor,
            ),
            (ColorBlendFactor::ConstantAlpha, BlendFactor::ConstantAlpha),
            (
                ColorBlendFactor::OneMinusConstantAlpha,
                BlendFactor::OneMinusConstantAlpha,
            ),
            (
                ColorBlendFactor::SourceAlphaSaturate,
                BlendFactor::SrcAlphaSaturate,
            ),
        ];

        for (public, vulkan) in factors {
            assert_eq!(public.to_vulkano(), vulkan);
        }
    }

    #[test]
    fn public_operations_map_to_vulkan_operations() {
        let operations = [
            (ColorBlendOperation::Add, BlendOp::Add),
            (ColorBlendOperation::Subtract, BlendOp::Subtract),
            (
                ColorBlendOperation::ReverseSubtract,
                BlendOp::ReverseSubtract,
            ),
            (ColorBlendOperation::Min, BlendOp::Min),
            (ColorBlendOperation::Max, BlendOp::Max),
        ];

        for (public, vulkan) in operations {
            assert_eq!(public.to_vulkano(), vulkan);
        }
    }
}
