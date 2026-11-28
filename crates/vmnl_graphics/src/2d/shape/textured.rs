// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Indexed 2D geometry with a managed texture color and alpha policy.

use super::validation::validate_positions;
use super::{Drawable2D, RenderItem2D, Shape, Vector2f};
use crate::common::{
    validate_indexed_triangle_geometry, BufferMemoryPreference, ColorBlendConfig,
    GraphicsResourceFactory, IndexBuffer, Rgba, VertexBuffer,
};
use crate::d2::Draw2D;
use crate::texture::{
    AddressMode, AlphaMode, ColorEncoding, Sampler, SamplerBorderColor, SamplerConfig,
    SamplerCoordinateMode, TextureFormat, TextureUploadDependency, TextureView2D,
    TextureViewInterpretation,
};
use crate::{
    ColorFormat, SurfaceColorPair, SurfaceColorSpace, VMNLError, VMNLErrorKind, VMNLResult, Window,
};
use std::sync::Arc;
use vulkano::{
    descriptor_set::{DescriptorImageViewInfo, DescriptorSet, WriteDescriptorSet},
    image::ImageLayout,
    pipeline::{GraphicsPipeline, Pipeline},
    render_pass::RenderPass,
};

/// Vertex of an indexed textured shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedVertex2D {
    /// Position in framebuffer pixels, with the origin at the top left.
    pub position: Vector2f,
    /// RGB modulation coefficients and linear opacity.
    pub color: Rgba,
    /// Finite normalized UV coordinates over the entire view. The sampler controls addressing.
    pub uv: [f32; 2],
}

#[repr(C)]
#[derive(crate::raw::Vertex, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub(crate) struct GpuTexturedVertex2D {
    #[format(R32G32_SFLOAT)]
    position: Vector2f,
    #[format(R32G32B32A32_SFLOAT)]
    modulation: [f32; 4],
    #[format(R32G32_SFLOAT)]
    uv: [f32; 2],
}

pub(crate) struct TexturedDraw {
    pub(crate) pipeline: Arc<GraphicsPipeline>,
    pub(crate) render_pass: Arc<RenderPass>,
    pub(crate) descriptors: Arc<DescriptorSet>,
    pub(crate) vertices: VertexBuffer<GpuTexturedVertex2D>,
    pub(crate) indices: IndexBuffer,
    pub(crate) index_count: u32,
    pub(crate) upload: TextureUploadDependency,
    pub(crate) alpha_mode: AlphaMode,
}

/// An indexed textured shape built for one window. Ordinary swapchain resize preserves its
/// pipeline. Submit it through [`crate::FrameRenderer::draw2d`].
pub struct TexturedShape {
    draw: Arc<TexturedDraw>,
    view: TextureView2D,
    sampler: SamplerConfig,
    tint: Rgba,
    modulation_encoding: ColorEncoding,
    blend: ColorBlendConfig,
}

impl TexturedShape {
    /// Borrows the texture view sampled by the shape.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn view(&self) -> &TextureView2D {
        &self.view
    }

    /// Returns the effective sampler configuration.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn sampler_config(&self) -> SamplerConfig {
        self.sampler
    }

    /// Returns the shape-wide modulation applied in addition to each vertex color.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn tint(&self) -> Rgba {
        self.tint
    }

    /// Returns the declared encoding of the supplied vertex and tint RGB values.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn modulation_encoding(&self) -> ColorEncoding {
        self.modulation_encoding
    }

    /// Returns the resolved GPU blend configuration.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn color_blend_config(&self) -> ColorBlendConfig {
        self.blend
    }
}

impl Drawable2D for TexturedShape {
    fn render_item_2d(&self) -> RenderItem2D {
        RenderItem2D {
            draw: Draw2D::Textured(self.draw.clone()),
        }
    }
}

impl GraphicsResourceFactory for TexturedShape {}

/// CPU-side configuration of an indexed textured shape.
pub struct TexturedShapeBuilder {
    vertices: Vec<TexturedVertex2D>,
    indices: Vec<u32>,
    view: TextureView2D,
    sampler: Option<Sampler>,
    tint: Rgba,
    modulation_encoding: ColorEncoding,
    blend: Option<ColorBlendConfig>,
    memory_preference: BufferMemoryPreference,
}

impl Shape {
    /// Configures a textured triangle list without creating GPU resources.
    ///
    /// # Arguments
    ///
    /// - `vertices` supplies framebuffer positions, normalized UVs, and RGBA modulation.
    /// - `indices` supplies consecutive triangles as groups of three vertex indices.
    /// - `view` supplies a view with a declared color and alpha interpretation. Use
    ///   [`crate::texture::Texture2D::view`] to obtain the texture's default view.
    ///
    /// # Example
    ///
    /// <a id="example-indexed-textured"></a>
    ///
    /// ```rust,no_run
    /// # use vmnl_graphics::{Window, VMNLResult};
    /// use vmnl_graphics::{common::Rgba, d2::{Shape, TexturedVertex2D, Vector2f},
    ///     texture::{Sampler, TextureView2D}};
    ///
    /// fn draw(window: &mut Window, view: &TextureView2D, sampler: &Sampler) -> VMNLResult<()> {
    ///     let vertex = |x, y, uv| TexturedVertex2D {
    ///         position: Vector2f { x, y },
    ///         color: Rgba::WHITE,
    ///         uv,
    ///     };
    ///     let shape = Shape::indexed_textured(
    ///         [vertex(0.0, 0.0, [0.0, 0.0]), vertex(100.0, 0.0, [1.0, 0.0]),
    ///          vertex(0.0, 100.0, [0.0, 1.0])],
    ///         [0, 1, 2],
    ///         view,
    ///     )
    ///     .sampler(sampler)
    ///     .tint([255, 192, 255, 192])
    ///     .build(window)?;
    ///     window.render().draw2d([&shape]).submit()
    /// }
    /// ```
    #[must_use]
    pub fn indexed_textured(
        vertices: impl IntoIterator<Item = TexturedVertex2D>,
        indices: impl IntoIterator<Item = u32>,
        view: &TextureView2D,
    ) -> TexturedShapeBuilder {
        TexturedShapeBuilder {
            vertices: vertices.into_iter().collect(),
            indices: indices.into_iter().collect(),
            view: view.clone(),
            sampler: None,
            tint: Rgba::WHITE,
            modulation_encoding: ColorEncoding::Linear,
            blend: None,
            memory_preference: BufferMemoryPreference::default(),
        }
    }
}

impl TexturedShapeBuilder {
    /// Sets the preferred vertex and index buffer placement. Defaults to device memory.
    ///
    /// # Arguments
    ///
    /// - `preference` supplies a placement preference for the two geometry buffers.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn buffer_memory_preference(mut self, preference: BufferMemoryPreference) -> Self {
        self.memory_preference = preference;
        self
    }

    /// Uses an existing sampler. The default builds a sampler with `SamplerConfig::default()`.
    ///
    /// # Arguments
    ///
    /// - `sampler` supplies a normalized-coordinate sampler from the window's context.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub fn sampler(mut self, sampler: &Sampler) -> Self {
        self.sampler = Some(sampler.clone());
        self
    }

    /// Sets shape-wide RGBA modulation. The default is white with full opacity.
    ///
    /// # Arguments
    ///
    /// - `tint` supplies RGB in the declared modulation encoding and linear alpha.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub fn tint(mut self, tint: impl Into<Rgba>) -> Self {
        self.tint = tint.into();
        self
    }

    /// Declares the vertex and tint RGB encoding. The default is `ColorEncoding::Linear`.
    ///
    /// # Arguments
    ///
    /// - `encoding` selects linear coefficients or explicit CPU sRGB decoding before
    ///   interpolation. Alpha is always linear.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn modulation_encoding(mut self, encoding: ColorEncoding) -> Self {
        self.modulation_encoding = encoding;
        self
    }

    /// Replaces the source-over blend configuration. The shader still emits the representation
    /// selected by the view's alpha declaration.
    ///
    /// # Arguments
    ///
    /// - `blend` supplies the complete color and alpha blend state for that shader output.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    #[must_use]
    pub const fn color_blend(mut self, blend: ColorBlendConfig) -> Self {
        self.blend = Some(blend);
        self
    }

    /// Builds indexed geometry, a graphics pipeline and a descriptor set for a window.
    ///
    /// Uses a native sRGB attachment with nonlinear sRGB presentation. Texture RGB is sampled
    /// as linear light, modulated and blended in linear light, then encoded by the attachment.
    /// This allocates GPU buffers and creates a sampler if none was supplied. The view shares
    /// its existing image storage. Submission chains any pending upload to the draw.
    ///
    /// # Arguments
    ///
    /// - `window` supplies the target and context. Use the shape on this window's thread.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedManagedColorTarget` for other target pairs, or `InvalidState` for
    /// invalid geometry, view interpretation, or sampler compatibility. Propagates backend
    /// resource creation errors.
    ///
    /// # Example
    ///
    /// See [the shared example](Shape#example-indexed-textured).
    pub fn build(self, window: &Window) -> VMNLResult<TexturedShape> {
        let index_count = validate_textured_geometry(&self.vertices, &self.indices)?;
        validate_target(window.surface_color_info().pair())?;
        let interpretation = self.view.sampled_interpretation().ok_or_else(|| {
            invalid_state("textured shape requires a declared color and alpha interpretation")
        })?;
        validate_view_encoding(self.view.effective_format(), interpretation)?;

        let context = window.resource_context();
        let image = self.view.sampled_image_binding();
        if !Arc::ptr_eq(&image.device, &context.inner.device) || !image.sampled_usage {
            return Err(invalid_state(
                "textured shape requires a sampleable view from its window context",
            ));
        }
        let alpha_mode = interpretation.alpha_mode();
        let config = self
            .sampler
            .as_ref()
            .map_or_else(SamplerConfig::default, Sampler::config);
        validate_sampler_config(config, alpha_mode)?;
        let sampler = match self.sampler {
            Some(sampler) => sampler,
            None => Sampler::builder(&context).config(config).build()?,
        };
        self.view.validate_sampler(&sampler)?;
        let blend = self.blend.unwrap_or_else(|| source_over(alpha_mode));
        let pipeline = window.create_textured_2d_pipeline(blend)?;
        let layout = pipeline
            .layout()
            .set_layouts()
            .first()
            .cloned()
            .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanPipelineLayoutCreationFailed))?;
        let descriptors = DescriptorSet::new(
            context.inner.descriptor_set_allocator.clone(),
            layout,
            [WriteDescriptorSet::image_view_with_layout_sampler(
                0,
                DescriptorImageViewInfo {
                    image_view: image.image_view,
                    image_layout: ImageLayout::General,
                },
                sampler.sampled_binding().sampler,
            )],
            [],
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanDescriptorSetCreationFailed))?;

        let tint = linear_modulation(self.tint, self.modulation_encoding);
        let allocator = &context.inner.memory_allocator;
        let vertices = TexturedShape::create_vertex_buffer(
            self.vertices.into_iter().map(|vertex| {
                let mut modulation = linear_modulation(vertex.color, self.modulation_encoding);
                for (value, tint) in modulation.iter_mut().zip(tint) {
                    *value *= tint;
                }
                GpuTexturedVertex2D {
                    position: vertex.position,
                    modulation,
                    uv: vertex.uv,
                }
            }),
            self.memory_preference,
            allocator,
        )?;
        let indices =
            TexturedShape::create_index_buffer(&self.indices, self.memory_preference, allocator)?;

        Ok(TexturedShape {
            draw: Arc::new(TexturedDraw {
                pipeline,
                render_pass: window.render_pass(),
                descriptors,
                vertices,
                indices,
                index_count,
                upload: image.upload_dependency,
                alpha_mode,
            }),
            view: self.view,
            sampler: config,
            tint: self.tint,
            modulation_encoding: self.modulation_encoding,
            blend,
        })
    }
}

fn invalid_state(message: &str) -> VMNLError {
    VMNLError::new(VMNLErrorKind::InvalidState(message.into()))
}

fn validate_textured_geometry(vertices: &[TexturedVertex2D], indices: &[u32]) -> VMNLResult<u32> {
    let (_, count) = validate_indexed_triangle_geometry(vertices.len(), indices, "textured shape")?;
    validate_positions(
        vertices.iter().map(|vertex| vertex.position),
        "textured shape",
    )?;
    if vertices
        .iter()
        .flat_map(|vertex| vertex.uv)
        .any(|value| !value.is_finite())
    {
        return Err(invalid_state(
            "textured shape UV coordinates must be finite",
        ));
    }
    Ok(count)
}

fn validate_target(pair: SurfaceColorPair) -> VMNLResult<()> {
    if pair.color_space() == SurfaceColorSpace::SRGB_NON_LINEAR
        && [ColorFormat::B8G8R8A8_SRGB, ColorFormat::R8G8B8A8_SRGB].contains(&pair.format())
    {
        Ok(())
    } else {
        Err(VMNLError::new(
            VMNLErrorKind::UnsupportedManagedColorTarget { pair },
        ))
    }
}

fn validate_view_encoding(
    format: TextureFormat,
    interpretation: TextureViewInterpretation,
) -> VMNLResult<()> {
    match (format, interpretation.encoding()) {
        (TextureFormat::Rgba8Srgb, ColorEncoding::Srgb)
        | (TextureFormat::Rgba8Unorm, ColorEncoding::Linear) => Ok(()),
        _ => Err(invalid_state(
            "textured shape view encoding must match the texture format's RGB decoding",
        )),
    }
}

fn validate_sampler_config(config: SamplerConfig, alpha: AlphaMode) -> VMNLResult<()> {
    if config.coordinate_mode() != SamplerCoordinateMode::Normalized {
        return Err(invalid_state(
            "indexed textured shapes require normalized sampler coordinates",
        ));
    }
    if alpha == AlphaMode::Opaque
        && [config.address_u(), config.address_v()].contains(&AddressMode::ClampToBorder)
        && matches!(
            config.border_color(),
            SamplerBorderColor::FloatTransparentBlack | SamplerBorderColor::IntTransparentBlack
        )
    {
        return Err(invalid_state(
            "an opaque texture interpretation cannot preserve a transparent sampler border",
        ));
    }
    Ok(())
}

fn source_over(alpha: AlphaMode) -> ColorBlendConfig {
    match alpha {
        AlphaMode::Premultiplied => ColorBlendConfig::premultiplied_alpha(),
        AlphaMode::Straight | AlphaMode::Opaque => ColorBlendConfig::straight_alpha(),
    }
}

fn linear_modulation(color: Rgba, encoding: ColorEncoding) -> [f32; 4] {
    let mut channels = color.normalized();
    if encoding == ColorEncoding::Srgb {
        for channel in &mut channels[..3] {
            *channel = if *channel <= 0.04045 {
                *channel / 12.92
            } else {
                ((*channel + 0.055) / 1.055).powf(2.4)
            };
        }
    }
    channels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_modulation_decodes_requested_srgb_and_keeps_alpha_linear() {
        let color = Rgba::new(128, 0, 255, 128);
        assert_eq!(
            linear_modulation(color, ColorEncoding::Linear).map(f32::to_bits),
            color.normalized().map(f32::to_bits)
        );
        let linear = linear_modulation(color, ColorEncoding::Srgb);
        assert!((linear[0] - 0.215_860_5).abs() < 0.000_001);
        assert_eq!(linear[1..], [0.0, 1.0, 128.0 / 255.0]);
    }

    #[test]
    fn managed_target_requires_both_an_srgb_attachment_and_nonlinear_srgb_presentation() {
        let space = SurfaceColorSpace::SRGB_NON_LINEAR;
        for format in [ColorFormat::B8G8R8A8_SRGB, ColorFormat::R8G8B8A8_SRGB] {
            assert!(validate_target(SurfaceColorPair::new(format, space)).is_ok());
        }
        for pair in [
            SurfaceColorPair::new(ColorFormat::B8G8R8A8_UNORM, space),
            SurfaceColorPair::new(ColorFormat::R8G8B8A8_UNORM, space),
            SurfaceColorPair::new(
                ColorFormat::B8G8R8A8_SRGB,
                SurfaceColorSpace(ash::vk::ColorSpaceKHR::DISPLAY_P3_NONLINEAR_EXT),
            ),
        ] {
            assert!(matches!(
                validate_target(pair),
                Err(error) if matches!(error.kind(), VMNLErrorKind::UnsupportedManagedColorTarget { pair: actual } if *actual == pair)
            ));
        }
    }

    #[test]
    fn managed_geometry_accepts_uv_repeats_and_preserves_validation_order() {
        let vertex = TexturedVertex2D {
            position: Vector2f { x: 0.0, y: 0.0 },
            color: Rgba::WHITE,
            uv: [-1.0, 2.0],
        };
        let mut vertices = [vertex; 3];
        assert!(validate_textured_geometry(&vertices, &[0, 0, 2]).is_ok());
        vertices[0].position.x = f32::INFINITY;
        vertices[1].position.y = f32::NAN;
        vertices[2].uv[0] = f32::NAN;
        for (indices, expected) in [
            ([0, 0, 3], "is out of bounds"),
            ([0, 1, 2], "positions must not be NaN"),
        ] {
            assert!(matches!(
                validate_textured_geometry(&vertices, &indices),
                Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message) if message.contains(expected))
            ));
        }
        vertices[0] = vertex;
        vertices[1] = vertex;
        assert!(matches!(
            validate_textured_geometry(&vertices, &[0, 1, 2]),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message) if message.contains("UV coordinates"))
        ));
    }

    #[test]
    fn managed_sampling_rejects_srgb_declarations_on_unorm_views() {
        let interpretation =
            TextureViewInterpretation::new(ColorEncoding::Srgb, AlphaMode::Straight);
        assert!(validate_view_encoding(TextureFormat::Rgba8Srgb, interpretation).is_ok());
        assert!(validate_view_encoding(TextureFormat::Rgba8Unorm, interpretation).is_err());
    }
}
