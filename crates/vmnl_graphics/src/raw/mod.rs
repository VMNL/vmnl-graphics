// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Low-level raw rendering API.

mod depth;
mod shader_compat;
mod viewport;
pub use depth::{CompareOp, DepthState};
pub(crate) use viewport::resolve_viewport_scissor;
pub use viewport::{Scissor, ScissorPolicy, Viewport, ViewportPolicy};

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};

pub use vmnl_macros::{Pod, Vertex, Zeroable};

use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::BufferContents as VulkanoBufferContents;
use vulkano::buffer::BufferUsage;
use vulkano::buffer::Subbuffer;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::{
    DescriptorSetLayout, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::descriptor_set::{DescriptorImageViewInfo, DescriptorSet, WriteDescriptorSet};
use vulkano::device::{Device, DeviceFeatures};
use vulkano::format::NumericType;
use vulkano::image::ImageLayout;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::color_blend::{
    AttachmentBlend, ColorBlendAttachmentState, ColorBlendState,
};
use vulkano::pipeline::graphics::input_assembly::{
    InputAssemblyState, PrimitiveTopology as VulkanoPrimitiveTopology,
};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::{
    CullMode as VulkanoCullMode, FrontFace as VulkanoFrontFace, PolygonMode as VulkanoPolygonMode,
    RasterizationState,
};
use vulkano::pipeline::graphics::vertex_input::{
    Vertex as VulkanoVertex, VertexBuffersCollection, VertexDefinition,
};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{
    DynamicState, GraphicsPipeline, Pipeline as VulkanoPipeline, PipelineLayout,
    PipelineShaderStageCreateInfo,
};
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::shader::{EntryPoint, ShaderModule, ShaderModuleCreateInfo};
use vulkano::sync::HostAccessError;

use crate::common::{
    checked_draw_counts, BufferMemoryPreference, ColorBlendConfig, ColorBlendOverrides,
    GraphicsResourceFactory, IndexBuffer, VertexBuffer,
};
use crate::exception::{VMNLError, VMNLErrorKind, VMNLResult};
use crate::texture::{
    validate_sampled_image_sampler, SampledImageBinding, SampledSamplerBinding,
    SampledTextureBinding, Sampler, Texture2D, TextureUploadDependency, TextureView2D,
};
use crate::{Context, DeviceFeature, LineWidthLimits, RenderTargetInfo, Window};
use shader_compat::{BindingPoint, SamplerOperation, ShaderContract, ShaderStageContract};

const FRAME_UNIFORM_IMAGE_COUNT_MISMATCH: &str =
    "raw frame uniform image count is incompatible with current swapchain image count";

type FrameUniformAllocator = SubbufferAllocator<StandardMemoryAllocator>;
type SharedFrameUniformSlots = Arc<Mutex<FrameUniformSlots>>;

#[doc(hidden)]
pub mod __private {
    pub use bytemuck;
    pub use vulkano;
}

/// Marker trait for raw buffer contents.
///
/// This is a public adapter for Vulkano's buffer-content contract. Incorrect
/// unsafe implementations of the underlying trait can make byte transfers
/// unsound; prefer VMNL's marker derives where applicable.
pub trait BufferContents: VulkanoBufferContents {}

impl<T> BufferContents for T where T: VulkanoBufferContents {}

/// Marker trait for raw vertex layouts.
///
/// The layout must match the vertex shader interface. Pipeline construction
/// validates the generated definition and returns an error on a mismatch.
pub trait Vertex: VulkanoVertex {}

impl<T> Vertex for T where T: VulkanoVertex {}

/// Marker trait for plain-old-data raw values.
///
/// This inherits bytemuck's safety contract: all bit patterns are valid and
/// the representation contains no uninitialized padding. Prefer `#[derive(Pod)]`.
pub trait Pod: bytemuck::Pod {}

impl<T> Pod for T where T: bytemuck::Pod {}

/// Marker trait for zero-initializable raw values.
///
/// This inherits bytemuck's requirement that the all-zero byte pattern is
/// valid. It does not by itself prove the stronger `Pod` contract.
pub trait Zeroable: bytemuck::Zeroable {}

impl<T> Zeroable for T where T: bytemuck::Zeroable {}

pub use crate::common::ShaderSource;

/// Primitive topology used by a raw pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveTopology {
    /// Independent points.
    PointList,
    /// Independent lines.
    LineList,
    /// Connected line strip.
    LineStrip,
    /// Independent triangles.
    TriangleList,
    /// Connected triangle strip.
    TriangleStrip,
}

impl From<PrimitiveTopology> for VulkanoPrimitiveTopology {
    fn from(value: PrimitiveTopology) -> Self {
        match value {
            PrimitiveTopology::PointList => Self::PointList,
            PrimitiveTopology::LineList => Self::LineList,
            PrimitiveTopology::LineStrip => Self::LineStrip,
            PrimitiveTopology::TriangleList => Self::TriangleList,
            PrimitiveTopology::TriangleStrip => Self::TriangleStrip,
        }
    }
}

/// Blend mode used by a raw pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// Disable blending and write fragment output directly.
    Opaque,
    /// Preserve the original source-alpha blend equation.
    ///
    /// RGB is `Cs * As + Cd * (1 - As)`; alpha is
    /// `As * As + Ad * (1 - As)`. Use [`ColorBlendConfig::straight_alpha`]
    /// for the common source-over alpha equation `As + Ad * (1 - As)`.
    Alpha,
}

/// Triangle faces discarded by a raw pipeline.
///
/// [`FrontFace`] determines which triangles are front-facing. Culling does not
/// discard point or line primitives. [`PipelineSpec`] defaults to [`Self::None`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CullMode {
    /// Keep both front-facing and back-facing triangles.
    None,
    /// Discard front-facing triangles.
    Front,
    /// Discard back-facing triangles.
    Back,
    /// Discard all triangles.
    FrontAndBack,
}

/// Triangle winding considered front-facing by a raw pipeline.
///
/// Winding is evaluated in framebuffer coordinates after the shader and viewport
/// transformations, not directly from the application's vertex positions.
/// This also controls the fragment shader's `gl_FrontFacing` input when culling
/// is disabled. [`PipelineSpec`] defaults to [`Self::CounterClockwise`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontFace {
    /// Treat triangles with positive signed framebuffer area as front-facing.
    CounterClockwise,
    /// Treat triangles with negative signed framebuffer area as front-facing.
    Clockwise,
}

/// Triangle rasterization mode, independent of the primitive topology.
///
/// Culling still applies to triangles before their edges or vertices are rasterized.
/// This setting has no effect on line/point input topologies, but its device-feature
/// requirements are still checked at build. The default is [`Self::Fill`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolygonMode {
    /// Rasterize the triangle's surface.
    Fill,
    /// Rasterize triangle edges; requires [`DeviceFeature::FillModeNonSolid`].
    Line,
    /// Rasterize triangle vertices; size is supplied by the shader's `gl_PointSize`.
    ///
    /// Requires [`DeviceFeature::FillModeNonSolid`]. Portability-subset devices also
    /// require `pointPolygons`, which VMNL cannot currently request through `DeviceConfig`.
    /// Such devices return an explicit `InvalidState` error at build.
    Point,
}

/// Builder/specification for a raw graphics pipeline.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::common::{ColorBlendConfig, ColorBlendOverrides};
/// use vmnl_graphics::raw::{BlendMode, CullMode, DepthState, FrontFace,
///     PipelineSpec, PolygonMode, PrimitiveTopology, ScissorPolicy, ShaderSource, ViewportPolicy};
///
/// let spec = PipelineSpec::<[f32; 2]>::default()
///     .vertex_shader(ShaderSource::Path("triangle.vert".into()))
///     .fragment_shader(ShaderSource::Path("triangle.frag".into()))
///     .topology(PrimitiveTopology::TriangleList)
///     .depth(DepthState::Disabled)
///     .cull_mode(CullMode::None)
///     .front_face(FrontFace::CounterClockwise)
///     .polygon_mode(PolygonMode::Fill)
///     .line_width(1.0)
///     .viewport(ViewportPolicy::FullFramebuffer)
///     .scissor(ScissorPolicy::FullFramebuffer);
/// assert_eq!(spec.topology_value(), PrimitiveTopology::TriangleList);
/// assert_eq!(spec.depth_value(), DepthState::Disabled);
/// assert_eq!(spec.cull_mode_value(), CullMode::None);
/// assert_eq!(spec.front_face_value(), FrontFace::CounterClockwise);
/// assert_eq!(spec.polygon_mode_value(), PolygonMode::Fill);
/// assert_eq!(spec.line_width_value(), 1.0);
/// assert_eq!(spec.viewport_value(), ViewportPolicy::FullFramebuffer);
/// assert_eq!(spec.scissor_value(), ScissorPolicy::FullFramebuffer);
///
/// let spec = spec.blend_mode(BlendMode::Opaque);
/// assert_eq!(spec.blend_mode_value(), BlendMode::Opaque);
/// let spec = spec.color_blend(ColorBlendConfig::straight_alpha());
/// assert!(spec.color_blend_config_value().blend_enabled());
/// let spec = spec.color_blend_overrides(ColorBlendOverrides::default().with_blend_enabled(false));
/// assert_eq!(spec.color_blend_overrides_value().blend_enabled(), Some(false));
/// ```
#[derive(Clone, Debug)]
pub struct PipelineSpec<TVertex> {
    vertex_shader: Option<ShaderSource>,
    fragment_shader: Option<ShaderSource>,
    topology: PrimitiveTopology,
    blend_mode: Option<BlendMode>,
    color_blend: Option<ColorBlendOverrides>,
    cull_mode: CullMode,
    front_face: FrontFace,
    polygon_mode: PolygonMode,
    line_width: f32,
    viewport: ViewportPolicy,
    scissor: ScissorPolicy,
    depth: DepthState,
    _vertex: PhantomData<TVertex>,
}

impl<TVertex> Default for PipelineSpec<TVertex> {
    fn default() -> Self {
        Self {
            vertex_shader: None,
            fragment_shader: None,
            topology: PrimitiveTopology::TriangleList,
            blend_mode: None,
            color_blend: None,
            cull_mode: CullMode::None,
            front_face: FrontFace::CounterClockwise,
            polygon_mode: PolygonMode::Fill,
            line_width: 1.0,
            viewport: ViewportPolicy::FullFramebuffer,
            scissor: ScissorPolicy::FullFramebuffer,
            depth: DepthState::Disabled,
            _vertex: PhantomData,
        }
    }
}

impl<TVertex> PipelineSpec<TVertex> {
    /// Sets immutable depth comparison/write policy; defaults to disabled.
    ///
    /// Any enabled test requires a window depth attachment. Validation occurs at
    /// [`Self::build`] before shader I/O; no attachment or feature is added implicitly.
    ///
    /// # Arguments
    ///
    /// - `state` selects depth comparison and writes. Enable a compatible depth attachment on the
    ///   target window. See [`DepthState`].
    ///
    /// # Example
    ///
    /// See [`depth` in the shared example](Self#example).
    #[must_use]
    pub fn depth(mut self, state: DepthState) -> Self {
        self.depth = state;
        self
    }

    /// Returns the configured depth policy without allocation or GPU access.
    ///
    /// # Example
    ///
    /// See [`depth_value` in the shared example](Self#example).
    #[must_use]
    pub const fn depth_value(&self) -> DepthState {
        self.depth
    }

    /// Sets the vertex shader.
    ///
    /// # Arguments
    ///
    /// - `source` supplies inline GLSL source or a GLSL file path for the vertex stage. See
    ///   [`ShaderSource`].
    ///
    /// # Example
    ///
    /// See [`vertex_shader` in the shared example](Self#example).
    #[must_use]
    pub fn vertex_shader(mut self, source: ShaderSource) -> Self {
        self.vertex_shader = Some(source);
        self
    }

    /// Sets the fragment shader.
    ///
    /// # Arguments
    ///
    /// - `source` supplies inline GLSL source or a GLSL file path for the fragment stage. See
    ///   [`ShaderSource`].
    ///
    /// # Example
    ///
    /// See [`fragment_shader` in the shared example](Self#example).
    #[must_use]
    pub fn fragment_shader(mut self, source: ShaderSource) -> Self {
        self.fragment_shader = Some(source);
        self
    }

    /// Sets the primitive topology.
    ///
    /// # Arguments
    ///
    /// - `topology` chooses how vertices or indices assemble into primitives. See
    ///   [`PrimitiveTopology`].
    ///
    /// # Example
    ///
    /// See [`topology` in the shared example](Self#example).
    #[must_use]
    pub fn topology(mut self, topology: PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    /// Sets the legacy blend mode and clears any explicit color blend override.
    ///
    /// This preserves the original `BlendMode` request and equation. Call
    /// [`Self::color_blend`] afterward to select an explicit configuration.
    ///
    /// # Arguments
    ///
    /// - `blend_mode` selects a legacy blending preset that replaces the existing explicit request.
    ///   See [`BlendMode`].
    ///
    /// # Example
    ///
    /// See [`blend_mode` in the shared example](Self#example).
    #[must_use]
    pub fn blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = Some(blend_mode);
        self.color_blend = None;
        self
    }

    /// Sets an explicit, editable color and alpha blend configuration.
    ///
    /// This overrides the legacy mode for pipeline creation without changing the
    /// value returned by [`Self::blend_mode_value`]. A later [`Self::blend_mode`]
    /// call clears this override.
    ///
    /// # Arguments
    ///
    /// - `config` supplies the full color and alpha blend state, for example from a
    ///   `ColorBlendConfig` preset.
    ///
    /// # Example
    ///
    /// See [`color_blend` in the shared example](Self#example).
    #[must_use]
    pub fn color_blend(mut self, config: ColorBlendConfig) -> Self {
        self.color_blend = Some(ColorBlendOverrides::from_config(config));
        self
    }

    /// Replaces the active blend request with per-field overrides.
    ///
    /// Unset fields use the window's context defaults at build. The legacy
    /// inspector still reports the last legacy selection, or `Opaque`.
    ///
    /// # Arguments
    ///
    /// - `overrides` supplies individual overrides to resolve against the context's raw blend
    ///   defaults.
    ///
    /// # Example
    ///
    /// See [`color_blend_overrides` in the shared example](Self#example).
    #[must_use]
    pub fn color_blend_overrides(mut self, overrides: ColorBlendOverrides) -> Self {
        self.color_blend = Some(overrides);
        self
    }

    /// Sets which triangle faces are discarded. Defaults to [`CullMode::None`].
    ///
    /// # Arguments
    ///
    /// - `cull_mode` selects which faces the rasterizer discards. See [`CullMode`].
    ///
    /// # Example
    ///
    /// See [`cull_mode` in the shared example](Self#example).
    #[must_use]
    pub fn cull_mode(mut self, cull_mode: CullMode) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    /// Sets the front-facing winding in framebuffer coordinates.
    /// Defaults to [`FrontFace::CounterClockwise`].
    ///
    /// # Arguments
    ///
    /// - `front_face` selects the winding treated as front-facing in framebuffer coordinates. See
    ///   [`FrontFace`].
    ///
    /// # Example
    ///
    /// See [`front_face` in the shared example](Self#example).
    #[must_use]
    pub fn front_face(mut self, front_face: FrontFace) -> Self {
        self.front_face = front_face;
        self
    }

    /// Sets triangle rasterization, fixed at pipeline creation. Defaults to [`PolygonMode::Fill`].
    ///
    /// [`Self::build`] validates the mode against the device features already enabled on the
    /// window's context.
    ///
    /// # Arguments
    ///
    /// - `mode` selects a [`PolygonMode`]. [`PolygonMode::Line`] and [`PolygonMode::Point`]
    ///   require [`DeviceFeature::FillModeNonSolid`] on the device. See [`PolygonMode::Point`]
    ///   for its additional portability constraint.
    ///
    /// # Example
    ///
    /// See [`polygon_mode` in the shared example](Self#example).
    #[must_use]
    pub fn polygon_mode(mut self, mode: PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    /// Sets line width in framebuffer units. Defaults to `1.0`; fixed at pipeline creation.
    ///
    /// Applies to line primitives and triangle edges in [`PolygonMode::Line`].
    /// Build requires a finite positive value within [`Context::line_width_limits`],
    /// and enabled [`DeviceFeature::WideLines`] for any value other than `1.0`.
    /// VMNL rejects out-of-range requests rather than clamping them. The driver may
    /// round an in-range width; no exact pixel thickness is promised.
    ///
    /// # Arguments
    ///
    /// - `width` supplies the finite positive width in framebuffer units. Check the device's
    ///   supported line-width limits.
    ///
    /// # Example
    ///
    /// See [`line_width` in the shared example](Self#example).
    #[must_use]
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Sets the viewport policy; defaults to the complete acquired framebuffer.
    ///
    /// Fixed rectangles use framebuffer pixels and persist across resize without scaling.
    /// Build validates them before shader I/O; recording resolves and revalidates the
    /// policy against the actual acquired image extent. No optional feature is activated.
    ///
    /// # Arguments
    ///
    /// - `policy` selects a framebuffer-sized viewport or an explicit viewport. See
    ///   [`ViewportPolicy`].
    ///
    /// # Example
    ///
    /// See [`viewport` in the shared example](Self#example).
    #[must_use]
    pub fn viewport(mut self, policy: ViewportPolicy) -> Self {
        self.viewport = policy;
        self
    }

    /// Sets the clipping policy, independently of the viewport transform.
    /// Defaults to [`ScissorPolicy::FullFramebuffer`].
    ///
    /// # Arguments
    ///
    /// - `policy` selects a framebuffer-sized scissor or an explicit clipping rectangle. See
    ///   [`ScissorPolicy`].
    ///
    /// # Example
    ///
    /// See [`scissor` in the shared example](Self#example).
    #[must_use]
    pub fn scissor(mut self, policy: ScissorPolicy) -> Self {
        self.scissor = policy;
        self
    }

    /// Returns the configured viewport policy; use [`ViewportPolicy::resolve`] to inspect it.
    ///
    /// # Example
    ///
    /// See [`viewport_value` in the shared example](Self#example).
    #[must_use]
    pub const fn viewport_value(&self) -> ViewportPolicy {
        self.viewport
    }

    /// Returns the configured scissor policy; use [`ScissorPolicy::resolve`] to inspect it.
    ///
    /// # Example
    ///
    /// See [`scissor_value` in the shared example](Self#example).
    #[must_use]
    pub const fn scissor_value(&self) -> ScissorPolicy {
        self.scissor
    }

    /// Returns the configured triangle rasterization mode without allocating.
    ///
    /// # Example
    ///
    /// See [`polygon_mode_value` in the shared example](Self#example).
    #[must_use]
    pub const fn polygon_mode_value(&self) -> PolygonMode {
        self.polygon_mode
    }

    /// Returns the requested line width, before device validation or driver rounding.
    ///
    /// # Example
    ///
    /// See [`line_width_value` in the shared example](Self#example).
    #[must_use]
    pub const fn line_width_value(&self) -> f32 {
        self.line_width
    }

    /// Returns the configured primitive topology.
    ///
    /// # Example
    ///
    /// See [`topology_value` in the shared example](Self#example).
    #[must_use]
    pub const fn topology_value(&self) -> PrimitiveTopology {
        self.topology
    }

    /// Returns the configured blend mode.
    ///
    /// # Example
    ///
    /// See [`blend_mode_value` in the shared example](Self#example).
    #[must_use]
    pub const fn blend_mode_value(&self) -> BlendMode {
        match self.blend_mode {
            Some(mode) => mode,
            None => BlendMode::Opaque,
        }
    }

    /// Resolves the specification against VMNL's opaque default, without a window.
    /// Use the built pipeline's getter for the actual state inherited from a context.
    ///
    /// # Example
    ///
    /// See [`color_blend_config_value` in the shared example](Self#example).
    #[must_use]
    pub const fn color_blend_config_value(&self) -> ColorBlendConfig {
        self.color_blend_overrides_value()
            .resolve(ColorBlendConfig::opaque())
    }

    /// Returns active local blend requests. A legacy mode or complete preset fixes every field.
    ///
    /// # Example
    ///
    /// See [`color_blend_overrides_value` in the shared example](Self#example).
    #[must_use]
    pub const fn color_blend_overrides_value(&self) -> ColorBlendOverrides {
        match self.color_blend {
            Some(config) => config,
            None => match self.blend_mode {
                Some(BlendMode::Opaque) => {
                    ColorBlendOverrides::from_config(ColorBlendConfig::opaque())
                }
                Some(BlendMode::Alpha) => {
                    ColorBlendOverrides::from_config(ColorBlendConfig::legacy_alpha())
                }
                None => ColorBlendOverrides::EMPTY,
            },
        }
    }

    /// Returns the configured triangle culling mode.
    ///
    /// # Example
    ///
    /// See [`cull_mode_value` in the shared example](Self#example).
    #[must_use]
    pub const fn cull_mode_value(&self) -> CullMode {
        self.cull_mode
    }

    /// Returns the configured front-facing winding.
    ///
    /// # Example
    ///
    /// See [`front_face_value` in the shared example](Self#example).
    #[must_use]
    pub const fn front_face_value(&self) -> FrontFace {
        self.front_face
    }

    /// Builds the raw pipeline against a window render pass.
    ///
    /// # Arguments
    ///
    /// - `window` supplies the device and render-pass identity against which this pipeline can draw.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for missing shader stages or unsupported shader declarations.
    /// Propagates shader I/O, compilation, and module creation errors. Returns
    /// `VulkanValidationFailed` for an incompatible vertex layout, and the corresponding
    /// `VulkanPipelineLayoutCreationFailed`, `VulkanRenderPassCreationFailed`, or
    /// `VulkanPipelineCreationFailed` if Vulkan pipeline setup fails.
    /// Before shader compilation, rejects invalid widths with `InvalidLineWidth`,
    /// disabled optional features with `DeviceFeatureNotEnabled`, and unsupported
    /// portability-subset point polygons or invalid viewport/scissor parameters
    /// (including device viewport limits) with `InvalidState`. No feature is activated
    /// and no GPU is reselected by this operation.
    /// An enabled depth test without a window depth attachment returns `InvalidState`
    /// before shader I/O, including `Always` tests with writes disabled.
    ///
    /// # Example
    ///
    /// <a id="example-pipeline-build"></a>
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, Window};
    /// use vmnl_graphics::raw::{self, Pipeline, PolygonMode, ShaderSource};
    ///
    /// #[repr(C)]
    /// #[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
    /// struct Vertex {
    ///     #[format(R32G32_SFLOAT)]
    ///     position: [f32; 2],
    /// }
    ///
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let window = Window::new(&context)?;
    /// let pipeline = Pipeline::<Vertex>::builder()
    ///     .vertex_shader(ShaderSource::Src(
    ///         "#version 450\nlayout(location=0) in vec2 position;\n\
    ///          void main() { gl_Position = vec4(position, 0.0, 1.0); }".into(),
    ///     ))
    ///     .fragment_shader(ShaderSource::Src(
    ///         "#version 450\nlayout(location=0) out vec4 color;\n\
    ///          void main() { color = vec4(1.0); }".into(),
    ///     ))
    ///     .build(&window)?;
    /// pipeline.validate_target(&window)?;
    /// assert_eq!(pipeline.render_target_info(), window.render_target_info());
    ///
    /// let (width, height) = window.get_framebuffer_size();
    /// if width > 0 && height > 0 {
    ///     let extent = [width, height];
    ///     let viewport = pipeline.viewport_value().resolve(extent)?;
    ///     let scissor = pipeline.scissor_value().resolve(extent)?;
    ///     assert_eq!(viewport.extent, [width as f32, height as f32]);
    ///     assert_eq!(scissor.extent, extent);
    /// }
    /// assert_eq!(pipeline.polygon_mode_value(), PolygonMode::Fill);
    /// assert_eq!(pipeline.line_width_value(), 1.0);
    /// assert!(!pipeline.color_blend_config_value().blend_enabled());
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self, window: &Window) -> VMNLResult<Pipeline<TVertex>>
    where
        TVertex: BufferContents + Vertex + 'static,
    {
        let device = window.device();
        validate_raw_rasterization(
            self.polygon_mode,
            self.line_width,
            LineWidthLimits::from_physical_device(device.physical_device()),
            device.enabled_features(),
            device.enabled_extensions().khr_portability_subset,
        )?;

        resolve_viewport_scissor(self.viewport, self.scissor, window.render_extent(), &device)?;
        let depth_stencil_state = self
            .depth
            .resolve(window.render_target_config().depth_value().is_some())?;
        let color_blend = self.color_blend_overrides_value().resolve(
            window
                .raw_color_blend_defaults()
                .resolve(ColorBlendConfig::opaque()),
        );

        let vertex_shader = self.vertex_shader.ok_or_else(|| {
            VMNLError::new(VMNLErrorKind::InvalidState(
                "raw pipeline requires a vertex shader".into(),
            ))
        })?;
        let fragment_shader = self.fragment_shader.ok_or_else(|| {
            VMNLError::new(VMNLErrorKind::InvalidState(
                "raw pipeline requires a fragment shader".into(),
            ))
        })?;

        let render_pass = window.render_pass();

        let (vs, fs, shader_contract) =
            compile_raw_shader_stages(device.clone(), &vertex_shader, &fragment_shader)?;

        let stages = [
            PipelineShaderStageCreateInfo::new(vs.clone()),
            PipelineShaderStageCreateInfo::new(fs),
        ];

        let layout_info = PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages);
        validate_raw_pipeline_layout(&layout_info)?;

        let layout = PipelineLayout::new(
            device.clone(),
            layout_info
                .into_pipeline_layout_create_info(device.clone())
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanPipelineLayoutCreationFailed))?,
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanPipelineLayoutCreationFailed))?;

        let subpass = Subpass::from(render_pass.clone(), 0)
            .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanRenderPassCreationFailed))?;
        let vertex_input_state = TVertex::per_vertex()
            .definition(&vs)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;

        let graphics_pipeline = GraphicsPipeline::new(
            device.clone(),
            None,
            vulkano::pipeline::graphics::GraphicsPipelineCreateInfo {
                stages: stages.into_iter().collect(),
                vertex_input_state: Some(vertex_input_state),
                input_assembly_state: Some(InputAssemblyState {
                    topology: self.topology.into(),
                    ..Default::default()
                }),
                viewport_state: Some(ViewportState::default()),
                rasterization_state: Some(raw_rasterization_state(
                    self.cull_mode,
                    self.front_face,
                    self.polygon_mode,
                    self.line_width,
                )),
                multisample_state: Some(MultisampleState::default()),
                depth_stencil_state,
                color_blend_state: Some(color_blend_state(color_blend)),
                dynamic_state: [DynamicState::Viewport, DynamicState::Scissor]
                    .into_iter()
                    .collect(),
                subpass: Some(subpass.into()),
                ..vulkano::pipeline::graphics::GraphicsPipelineCreateInfo::layout(layout)
            },
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanPipelineCreationFailed))?;

        Ok(Pipeline {
            inner: graphics_pipeline,
            device,
            render_pass,
            render_target_info: window.render_target_info(),
            polygon_mode: self.polygon_mode,
            line_width: self.line_width,
            viewport: self.viewport,
            scissor: self.scissor,
            depth: self.depth,
            color_blend,
            shader_contract,
            _vertex: PhantomData,
        })
    }
}

/// Raw graphics pipeline.
pub struct Pipeline<TVertex> {
    inner: Arc<GraphicsPipeline>,
    device: Arc<Device>,
    render_pass: Arc<RenderPass>,
    render_target_info: RenderTargetInfo,
    polygon_mode: PolygonMode,
    line_width: f32,
    viewport: ViewportPolicy,
    scissor: ScissorPolicy,
    depth: DepthState,
    color_blend: ColorBlendConfig,
    shader_contract: Arc<ShaderContract>,
    _vertex: PhantomData<TVertex>,
}

impl<TVertex> Pipeline<TVertex> {
    /// Returns the attachment properties of the window used at pipeline creation.
    ///
    /// This is a copied description, without extent, clear value or target identity.
    /// It allocates nothing and performs no GPU access. Equal descriptions do not
    /// make a pipeline compatible with another window; use [`Self::validate_target`].
    ///
    /// # Example
    ///
    /// See [`render_target_info` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub const fn render_target_info(&self) -> RenderTargetInfo {
        self.render_target_info
    }

    /// Checks the same device/render-pass identity rule used by raw submission.
    ///
    /// CPU-only, without GPU query, synchronization or shader work. Success
    /// allocates nothing; an error message may allocate.
    /// Resize preserves the originating window's render pass, so it preserves this
    /// check. This does not validate geometry, resources, viewport/scissor bounds or
    /// framebuffer readiness; submission still validates those separately.
    ///
    /// # Arguments
    ///
    /// - `window` supplies the intended raw draw target, usually the window used to build the
    ///   pipeline.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for any other device or render-pass object, including
    /// another window on the same context with identical attachment properties.
    /// Build a separate pipeline for that window.
    ///
    /// # Example
    ///
    /// See [`validate_target` in the shared example](PipelineSpec#example-pipeline-build).
    pub fn validate_target(&self, window: &Window) -> VMNLResult<()> {
        if !pipeline_targets_window(
            &self.device,
            &self.render_pass,
            &window.device(),
            &window.render_pass(),
        ) {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "raw pipeline must target this window context and render pass".into(),
            )));
        }
        Ok(())
    }

    /// Returns the immutable depth comparison/write policy used at creation.
    ///
    /// # Example
    ///
    /// Check that a pipeline used for a depth-writing pass has writes enabled.
    ///
    /// ```rust
    /// use vmnl_graphics::raw::{DepthState, Pipeline};
    ///
    /// fn require_depth_writes<V>(pipeline: &Pipeline<V>) -> Result<(), &'static str> {
    ///     match pipeline.depth_value() {
    ///         DepthState::Test { write_enable: true, .. } => Ok(()),
    ///         _ => Err("This pass requires depth writes"),
    ///     }
    /// }
    /// ```
    #[must_use]
    pub const fn depth_value(&self) -> DepthState {
        self.depth
    }

    /// Returns the effective color and alpha blend state used by this pipeline.
    ///
    /// This is the configuration passed to Vulkan when the pipeline was built,
    /// including any explicit override or legacy preset resolution. It performs
    /// no allocation or GPU access.
    ///
    /// # Example
    ///
    /// See [`color_blend_config_value` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub const fn color_blend_config_value(&self) -> ColorBlendConfig {
        self.color_blend
    }

    /// Starts a raw pipeline builder.
    ///
    /// # Example
    ///
    /// See [`builder` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub fn builder() -> PipelineSpec<TVertex> {
        PipelineSpec::default()
    }

    /// Returns the triangle rasterization mode used to create this pipeline.
    ///
    /// # Example
    ///
    /// See [`polygon_mode_value` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub const fn polygon_mode_value(&self) -> PolygonMode {
        self.polygon_mode
    }

    /// Returns the validated line width passed to Vulkan, before driver rounding.
    ///
    /// This is a configuration value, not a measurement of displayed pixel coverage.
    ///
    /// # Example
    ///
    /// See [`line_width_value` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub const fn line_width_value(&self) -> f32 {
        self.line_width
    }

    /// Returns the immutable viewport policy; resolution remains extent-dependent.
    ///
    /// # Example
    ///
    /// See [`viewport_value` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub const fn viewport_value(&self) -> ViewportPolicy {
        self.viewport
    }

    /// Returns the immutable scissor policy; resolution remains extent-dependent.
    ///
    /// # Example
    ///
    /// See [`scissor_value` in the shared example](PipelineSpec#example-pipeline-build).
    #[must_use]
    pub const fn scissor_value(&self) -> ScissorPolicy {
        self.scissor
    }

    /// Captures the pipeline and geometry for a pending raw draw without descriptors.
    ///
    /// Submission checks that both resources are compatible with the target window before recording.
    pub(crate) fn render_item(&self, geometry: &Geometry<TVertex>) -> RenderItemRaw {
        self.render_item_with_resources(geometry, None)
    }

    /// Captures the resources, pipeline, and geometry for a pending raw draw.
    ///
    /// Submission checks device and pipeline layout compatibility before recording.
    pub(crate) fn render_item_with(
        &self,
        geometry: &Geometry<TVertex>,
        resources: &Resources,
    ) -> RenderItemRaw {
        self.render_item_with_resources(geometry, Some(resources))
    }

    fn render_item_with_resources(
        &self,
        geometry: &Geometry<TVertex>,
        resources: Option<&Resources>,
    ) -> RenderItemRaw {
        RenderItemRaw {
            pipeline: self.inner.clone(),
            pipeline_device: self.device.clone(),
            pipeline_render_pass: self.render_pass.clone(),
            viewport: self.viewport,
            scissor: self.scissor,
            geometry_device: geometry.device.clone(),
            resources_device: resources.map(|resources| resources.device.clone()),
            resources_pipeline_layout: resources.map(|resources| resources.pipeline_layout.clone()),
            descriptor_sets: resources
                .map_or_else(RawDescriptorSets::empty, Resources::descriptor_sets),
            required_descriptor_set_count: required_descriptor_set_count(self.inner.layout()),
            vertex_buffer: geometry.vertex_buffer_bytes(),
            index_buffer: geometry.index_buffer(),
            vertex_count: geometry.vertex_count,
            index_count: geometry.index_count,
        }
    }
}

/// Checks exact device and render-pass identity for a pending raw draw.
///
/// The identities must match even when attachment properties compare equal.
pub(crate) fn pipeline_targets_window(
    pipeline_device: &Arc<Device>,
    pipeline_render_pass: &Arc<RenderPass>,
    window_device: &Arc<Device>,
    window_render_pass: &Arc<RenderPass>,
) -> bool {
    Arc::ptr_eq(pipeline_device, window_device)
        && Arc::ptr_eq(pipeline_render_pass, window_render_pass)
}

/// Raw typed geometry.
///
/// # Example
///
/// ```rust,no_run
/// use vmnl_graphics::{Context, common::BufferMemoryPreference};
/// use vmnl_graphics::raw::{self, Geometry};
///
/// #[repr(C)]
/// #[derive(Clone, Copy, raw::Pod, raw::Zeroable)]
/// struct Vertex {
///     position: [f32; 2],
/// }
///
/// # fn main() -> vmnl_graphics::VMNLResult<()> {
/// let context = Context::new()?;
/// let triangle = [[-0.5, -0.5], [0.5, -0.5], [0.0, 0.5]];
/// let _geometry = Geometry::builder(triangle.map(|position| Vertex { position }))
///     .indices([0, 1, 2])
///     .buffer_memory_preference(BufferMemoryPreference::Host)
///     .build(&context)?;
/// # Ok(())
/// # }
/// ```
pub struct Geometry<TVertex> {
    vertex_buffer: VertexBuffer<TVertex>,
    index_buffer: Option<IndexBuffer>,
    vertex_count: u32,
    index_count: u32,
    device: Arc<Device>,
}

impl<TVertex> Geometry<TVertex> {
    /// Starts a raw geometry builder.
    ///
    /// # Arguments
    ///
    /// - `vertices` supplies typed vertices as an array or vector. Use the layout expected by the raw
    ///   pipeline.
    ///
    /// # Example
    ///
    /// See [`builder` in the shared example](Self#example).
    #[must_use]
    pub fn builder<V>(vertices: V) -> GeometryBuilder<TVertex>
    where
        V: Into<Vec<TVertex>>,
    {
        GeometryBuilder {
            vertices: vertices.into(),
            indices: None,
            memory_preference: BufferMemoryPreference::default(),
        }
    }

    fn vertex_buffer_bytes(&self) -> Subbuffer<[u8]> {
        let mut buffers = self.vertex_buffer.as_subbuffer().into_vec();
        debug_assert_eq!(buffers.len(), 1);
        buffers.remove(0)
    }

    fn index_buffer(&self) -> Option<Subbuffer<[u32]>> {
        self.index_buffer.as_ref().map(IndexBuffer::as_subbuffer)
    }
}

/// Builder for raw typed geometry.
pub struct GeometryBuilder<TVertex> {
    vertices: Vec<TVertex>,
    indices: Option<Vec<u32>>,
    memory_preference: BufferMemoryPreference,
}

impl<TVertex> GeometryBuilder<TVertex> {
    /// Sets the index list used for indexed draws. Omit this call to draw vertices in array order.
    ///
    /// # Arguments
    ///
    /// - `indices` supplies zero-based references into the vertex array in the order required by
    ///   the pipeline topology. Pass a nonempty array or vector. [`Self::build`] checks index
    ///   bounds and draw counts.
    ///
    /// # Example
    ///
    /// See [`indices` in the shared example](Geometry#example).
    #[must_use]
    pub fn indices<I>(mut self, indices: I) -> Self
    where
        I: Into<Vec<u32>>,
    {
        self.indices = Some(indices.into());
        self
    }

    /// Sets the buffer memory preference.
    ///
    /// # Arguments
    ///
    /// - `preference` selects preferred host or device placement for the geometry buffers. See
    ///   [`BufferMemoryPreference`].
    ///
    /// # Example
    ///
    /// See [`buffer_memory_preference` in the shared example](Geometry#example).
    #[must_use]
    pub fn buffer_memory_preference(mut self, preference: BufferMemoryPreference) -> Self {
        self.memory_preference = preference;
        self
    }

    /// Builds vertex and optional index buffers on the supplied context's device.
    ///
    /// Consumes the builder and validates the list before creating either buffer. Use vertex
    /// data implementing [`BufferContents`], for example through VMNL's `Pod` derive.
    ///
    /// # Arguments
    ///
    /// - `context` supplies the device and allocator. Use the context associated with the target
    ///   pipeline.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for an empty vertex list, a supplied empty index list,
    /// an out-of-bounds index, or counts that cannot be represented as `u32`.
    /// Returns `VulkanVertexBufferCreationFailed` or `VulkanIndexBufferCreationFailed`
    /// if creating the corresponding buffer from the supplied data fails.
    ///
    /// # Example
    ///
    /// See [`build` in the shared example](Geometry#example).
    pub fn build(self, context: &Context) -> VMNLResult<Geometry<TVertex>>
    where
        TVertex: BufferContents,
    {
        let Self {
            vertices,
            indices,
            memory_preference,
        } = self;

        validate_geometry_inputs(vertices.len(), indices.as_deref())?;
        let (vertex_count, index_count) =
            checked_draw_counts(vertices.len(), indices.as_ref().map_or(0, Vec::len))?;

        let memory_allocator = &context.inner.memory_allocator;
        let vertex_buffer = <Self as GraphicsResourceFactory>::create_vertex_buffer(
            vertices,
            memory_preference,
            memory_allocator,
        )?;
        let index_buffer = match indices {
            Some(indices) => Some(<Self as GraphicsResourceFactory>::create_index_buffer(
                &indices,
                memory_preference,
                memory_allocator,
            )?),
            None => None,
        };

        Ok(Geometry {
            vertex_buffer,
            index_buffer,
            vertex_count,
            index_count,
            device: context.inner.device.clone(),
        })
    }
}

impl<TVertex> GraphicsResourceFactory for GeometryBuilder<TVertex> {}

/// Typed raw uniform buffer.
///
/// # Example
///
/// ```rust,no_run
/// use vmnl_graphics::{Context, common::BufferMemoryPreference};
/// use vmnl_graphics::raw::Uniform;
///
/// # fn main() -> vmnl_graphics::VMNLResult<()> {
/// let context = Context::new()?;
/// // This RGBA vector matches a GLSL vec4 in the bound uniform block.
/// let mut color = Uniform::builder([1.0_f32, 0.0, 0.0, 1.0])
///     .buffer_memory_preference(BufferMemoryPreference::Host)
///     .build(&context)?;
/// // Update before submitting work that reads the buffer.
/// color.write([0.0, 1.0, 0.0, 1.0])?;
/// # Ok(())
/// # }
/// ```
pub struct Uniform<TData> {
    buffer: Subbuffer<TData>,
    device: Arc<Device>,
}

impl<TData> Uniform<TData> {
    /// Starts a raw uniform buffer builder.
    ///
    /// # Arguments
    ///
    /// - `data` supplies the initial shader-visible value. Pass a type that implements
    ///   [`BufferContents`] at build time and matches the shader's uniform block layout.
    ///   For example, `[f32; 4]` can represent a block containing a single `vec4`.
    ///
    /// # Example
    ///
    /// See [`builder` in the shared example](Self#example).
    #[must_use]
    pub fn builder(data: TData) -> UniformBuilder<TData> {
        UniformBuilder {
            data,
            memory_preference: BufferMemoryPreference::default(),
        }
    }

    /// Writes a new value into the existing uniform buffer.
    ///
    /// This updates the same buffer object that already-built [`Resources`]
    /// reference. It does not recreate descriptor sets, submit GPU work, or wait
    /// for in-flight GPU access.
    ///
    /// # Arguments
    ///
    /// - `data` supplies the replacement value with the same Rust and shader layout as the original
    ///   value.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` if the buffer is currently locked or still used by
    /// the GPU. Returns a Vulkan validation error for other backend write
    /// failures.
    ///
    /// # Example
    ///
    /// See [`write` in the shared example](Self#example).
    pub fn write(&mut self, data: TData) -> VMNLResult<()>
    where
        TData: BufferContents,
    {
        {
            let mut guard = self
                .buffer
                .write()
                .map_err(|error| map_uniform_write_error(&error))?;
            *guard = data;
        }

        Ok(())
    }

    fn buffer_bytes(&self) -> Subbuffer<[u8]> {
        self.buffer.clone().into_bytes()
    }
}

/// Builder for typed raw uniform buffers.
pub struct UniformBuilder<TData> {
    data: TData,
    memory_preference: BufferMemoryPreference,
}

impl<TData> UniformBuilder<TData> {
    /// Sets the buffer memory preference.
    ///
    /// # Arguments
    ///
    /// - `preference` selects preferred host or device placement for the uniform buffer. See
    ///   [`BufferMemoryPreference`].
    ///
    /// # Example
    ///
    /// See [`buffer_memory_preference` in the shared example](Uniform#example).
    #[must_use]
    pub fn buffer_memory_preference(mut self, preference: BufferMemoryPreference) -> Self {
        self.memory_preference = preference;
        self
    }

    /// Builds a GPU uniform buffer.
    ///
    /// # Arguments
    ///
    /// - `context` supplies the device and allocator. Use the same context as the pipeline that will
    ///   bind this uniform.
    ///
    /// # Errors
    ///
    /// Returns `VulkanFrameUboBufferCreationFailed` if creating the uniform buffer
    /// from the supplied data fails.
    ///
    /// # Example
    ///
    /// See [`build` in the shared example](Uniform#example).
    pub fn build(self, context: &Context) -> VMNLResult<Uniform<TData>>
    where
        TData: BufferContents,
    {
        let buffer = <Self as GraphicsResourceFactory>::create_buffer_from_data(
            self.data,
            BufferUsage::UNIFORM_BUFFER,
            self.memory_preference,
            &context.inner.memory_allocator,
            VMNLErrorKind::VulkanFrameUboBufferCreationFailed,
        )?;

        Ok(Uniform {
            buffer,
            device: context.inner.device.clone(),
        })
    }
}

impl<TData> GraphicsResourceFactory for UniformBuilder<TData> {}

/// Typed raw uniform buffers for data that can change every frame.
///
/// Use `FrameUniform` for data that changes every frame, such as time, camera
/// matrices, transforms, tint, or offsets. The value is written through
/// [`FrameRenderer::write_frame_uniform`](crate::FrameRenderer::write_frame_uniform)
/// during frame submission so VMNL can target the acquired swapchain image
/// without exposing the image index to client code. Each write allocates or
/// reuses a fresh subbuffer for that frame slot, so it does not overwrite a
/// buffer that may still be read by queued GPU work.
///
/// # Example
///
/// ```rust,no_run
/// use vmnl_graphics::{Context, Window, common::BufferMemoryPreference};
/// use vmnl_graphics::raw::FrameUniform;
///
/// # fn main() -> vmnl_graphics::VMNLResult<()> {
/// let context = Context::new()?;
/// let mut window = Window::new(&context)?;
/// // The uniform block declares a vec4 at set 0, binding 0.
/// let mut color = FrameUniform::builder([1.0_f32, 0.0, 0.0, 1.0])
///     .buffer_memory_preference(BufferMemoryPreference::Host)
///     .build(&window)?;
/// let _frame = window.render()
///     .write_frame_uniform(&mut color, [0.0, 1.0, 0.0, 1.0]);
/// # Ok(())
/// # }
/// ```
pub struct FrameUniform<TData> {
    allocator: Arc<Mutex<FrameUniformAllocator>>,
    device: Arc<Device>,
    image_count: usize,
    slots: SharedFrameUniformSlots,
    _data: PhantomData<TData>,
}

impl<TData> FrameUniform<TData> {
    /// Starts a raw frame-uniform builder.
    ///
    /// # Arguments
    ///
    /// - `data` supplies the initial value copied into each swapchain image's slot. Pass a
    ///   cloneable type implementing [`BufferContents`] with the layout declared in the shader's
    ///   uniform block. See [`Uniform`] for an example value layout.
    ///
    /// # Example
    ///
    /// See [`builder` in the shared example](Self#example).
    #[must_use]
    pub fn builder(data: TData) -> FrameUniformBuilder<TData> {
        FrameUniformBuilder {
            data,
            memory_preference: BufferMemoryPreference::default(),
        }
    }

    fn buffer_slots(&self) -> SharedFrameUniformSlots {
        self.slots.clone()
    }

    fn image_count(&self) -> usize {
        self.image_count
    }

    fn write_slot(&mut self, image_index: usize, data: TData) -> VMNLResult<()>
    where
        TData: BufferContents,
    {
        if image_index >= self.image_count {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "raw frame uniform image index is out of bounds".into(),
            )));
        }

        let buffer = {
            let allocator = lock_frame_uniform_allocator(&self.allocator)?;
            allocate_frame_uniform_buffer(&allocator, data)?.into_bytes()
        };
        let mut slots = lock_frame_uniform_slots(&self.slots)?;
        let slot = slots.buffers.get_mut(image_index).ok_or_else(|| {
            VMNLError::new(VMNLErrorKind::InvalidState(
                "raw frame uniform image index is out of bounds".into(),
            ))
        })?;
        *slot = buffer;

        Ok(())
    }
}

struct FrameUniformSlots {
    buffers: Vec<Subbuffer<[u8]>>,
}

/// Builder for typed raw frame-uniform buffers.
pub struct FrameUniformBuilder<TData> {
    data: TData,
    memory_preference: BufferMemoryPreference,
}

impl<TData> FrameUniformBuilder<TData> {
    /// Sets the buffer memory preference.
    ///
    /// # Arguments
    ///
    /// - `preference` selects preferred host or device placement for the frame uniform slots. See
    ///   [`BufferMemoryPreference`].
    ///
    /// # Example
    ///
    /// See [`buffer_memory_preference` in the shared example](FrameUniform#example).
    #[must_use]
    pub fn buffer_memory_preference(mut self, preference: BufferMemoryPreference) -> Self {
        self.memory_preference = preference;
        self
    }

    /// Builds initial GPU uniform slots for the current swapchain images.
    ///
    /// # Arguments
    ///
    /// - `window` supplies the device and swapchain image count for the slots used by this window.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` if the window has no swapchain images, the allocator lock is
    /// poisoned, or a slot write conflicts with another access. Returns
    /// `VulkanFrameUboBufferCreationFailed` if allocating a slot fails, or
    /// `VulkanValidationFailed` for other host write errors.
    ///
    /// # Example
    ///
    /// See [`build` in the shared example](FrameUniform#example).
    pub fn build(self, window: &Window) -> VMNLResult<FrameUniform<TData>>
    where
        TData: BufferContents + Clone,
    {
        let image_count = window.swapchain_image_count();
        if image_count == 0 {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "raw frame uniform requires at least one swapchain image".into(),
            )));
        }

        let memory_allocator = window.memory_allocator();
        let allocator = Arc::new(Mutex::new(FrameUniformAllocator::new(
            memory_allocator,
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: self.memory_preference.memory_type_filter(),
                ..Default::default()
            },
        )));
        let mut buffers = Vec::with_capacity(image_count);
        {
            let allocator = lock_frame_uniform_allocator(&allocator)?;
            for _ in 0..image_count {
                buffers.push(allocate_frame_uniform_buffer(
                    &allocator,
                    self.data.clone(),
                )?);
            }
        }

        Ok(FrameUniform {
            allocator,
            device: window.device(),
            image_count,
            slots: Arc::new(Mutex::new(FrameUniformSlots {
                buffers: buffers.into_iter().map(Subbuffer::into_bytes).collect(),
            })),
            _data: PhantomData,
        })
    }
}

impl<TData> GraphicsResourceFactory for FrameUniformBuilder<TData> {}

/// Descriptor resources bound by a raw draw call.
///
/// This handle implements `Send + Sync` and may be moved or shared between Rust threads. This
/// does not make [`Context`] or [`Window`] thread-safe; operations that use them remain subject to
/// their thread-affinity requirements.
///
/// # Example
///
/// ```rust,no_run
/// use vmnl_graphics::{Context, VMNLResult};
/// use vmnl_graphics::raw::{FrameUniform, Pipeline, Resources, Uniform};
/// use vmnl_graphics::texture::{Sampler, Texture2D, TextureView2D};
///
/// // Each pipeline is built with the descriptor layout used by its helper below.
/// // The resources and pipelines come from the same context.
/// fn bind_uniforms<V>(
///     context: &Context,
///     pipeline: &Pipeline<V>,
///     frame_color: &FrameUniform<[f32; 4]>,
///     tint: &Uniform<[f32; 4]>,
/// ) -> VMNLResult<Resources> {
///     Resources::builder(pipeline)
///         .frame_uniform(0, 0, frame_color)
///         .uniform(0, 1, tint)
///         .build(context)
/// }
///
/// fn bind_combined<V>(
///     context: &Context,
///     pipeline: &Pipeline<V>,
///     texture: &Texture2D,
///     view: &TextureView2D,
///     sampler: &Sampler,
/// ) -> VMNLResult<Resources> {
///     Resources::builder(pipeline)
///         .sampled_texture(0, 0, texture, sampler)
///         .sampled_texture_view(0, 1, view, sampler)
///         .build(context)
/// }
///
/// fn bind_separate<V>(
///     context: &Context,
///     pipeline: &Pipeline<V>,
///     texture: &Texture2D,
///     view: &TextureView2D,
///     sampler: &Sampler,
/// ) -> VMNLResult<Resources> {
///     Resources::builder(pipeline)
///         .texture(0, 0, texture)
///         .texture_view(0, 1, view)
///         .sampler(0, 2, sampler)
///         .build(context)
/// }
/// ```
pub struct Resources {
    device: Arc<Device>,
    pipeline_layout: Arc<PipelineLayout>,
    descriptor_sets: RawDescriptorSets,
}

impl Resources {
    /// Starts a raw resource builder for a specific pipeline layout.
    ///
    /// # Arguments
    ///
    /// - `pipeline` supplies the descriptor layout and device for these bindings.
    ///
    /// # Example
    ///
    /// See [`builder` in the shared example](Self#example).
    #[must_use]
    pub fn builder<TVertex>(pipeline: &Pipeline<TVertex>) -> ResourcesBuilder {
        let pipeline_layout = pipeline.inner.layout().clone();
        ResourcesBuilder {
            pipeline_device: pipeline.device.clone(),
            pipeline_layout: pipeline_layout.clone(),
            set_layouts: pipeline_layout.set_layouts().to_vec(),
            shader_contract: pipeline.shader_contract.clone(),
            bindings: BTreeMap::new(),
            duplicate_binding: None,
            frame_uniform_image_count: None,
            incompatible_frame_uniform_image_count: None,
        }
    }

    fn descriptor_sets(&self) -> RawDescriptorSets {
        self.descriptor_sets.clone()
    }
}

/// Builder for raw descriptor resources.
///
/// Take each `set` and `binding` pair from the pipeline's shader declarations. Use one binding
/// per pair and choose the method that matches the shader's descriptor kind.
/// [`Self::build`] validates bindings before creating descriptor sets. Frame-uniform slots must
/// agree on the swapchain image count, which is checked again at frame submission.
///
/// This handle implements `Send + Sync` and may be moved or shared between Rust threads. This
/// does not make [`Context`] or [`Window`] thread-safe; building or using resources still requires
/// the owning context and window on their permitted threads.
pub struct ResourcesBuilder {
    pipeline_device: Arc<Device>,
    pipeline_layout: Arc<PipelineLayout>,
    set_layouts: Vec<Arc<DescriptorSetLayout>>,
    shader_contract: Arc<ShaderContract>,
    bindings: BTreeMap<u32, BTreeMap<u32, ResourceBinding>>,
    duplicate_binding: Option<(u32, u32)>,
    frame_uniform_image_count: Option<usize>,
    incompatible_frame_uniform_image_count: Option<(usize, usize, u32, u32)>,
}

impl ResourcesBuilder {
    /// Binds frame-uniform slots to a shader descriptor binding.
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the frame-uniform descriptor within that set.
    /// - `uniform` supplies slots from [`FrameUniformBuilder::build`] for the window that will
    ///   submit the draw. The uniform's data layout must match the declared block.
    ///
    /// # Example
    ///
    /// See [`frame_uniform` in the shared example](Resources#example).
    #[must_use]
    pub fn frame_uniform<TData>(
        mut self,
        set: u32,
        binding: u32,
        uniform: &FrameUniform<TData>,
    ) -> Self {
        let image_count = uniform.image_count();
        if let Some(mismatch) = next_frame_uniform_image_count_mismatch(
            &mut self.frame_uniform_image_count,
            image_count,
            set,
            binding,
        ) {
            if self.incompatible_frame_uniform_image_count.is_none() {
                self.incompatible_frame_uniform_image_count = Some(mismatch);
            }
        }

        let old = self.bindings.entry(set).or_default().insert(
            binding,
            ResourceBinding::FrameUniformBuffer {
                slots: uniform.buffer_slots(),
                device: uniform.device.clone(),
                image_count,
            },
        );
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Binds a uniform buffer to a shader descriptor binding.
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the uniform descriptor within that set.
    /// - `uniform` supplies a buffer from [`UniformBuilder::build`] using the pipeline's context
    ///   and a value layout matching the declared block.
    ///
    /// # Example
    ///
    /// See [`uniform` in the shared example](Resources#example).
    #[must_use]
    pub fn uniform<TData>(mut self, set: u32, binding: u32, uniform: &Uniform<TData>) -> Self {
        let old = self.bindings.entry(set).or_default().insert(
            binding,
            ResourceBinding::UniformBuffer {
                buffer: uniform.buffer_bytes(),
                device: uniform.device.clone(),
            },
        );
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Binds a texture's default view and sampler to a combined image-sampler descriptor.
    ///
    /// The texture image must include sampled usage, and the texture and sampler must
    /// belong to the same `Context` as the pipeline. A pending initial upload is joined
    /// into each frame that consumes these resources; this adds no CPU wait for that upload.
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the combined image-sampler descriptor within that set.
    /// - `texture` supplies the default texture view from the pipeline's context.
    /// - `sampler` supplies a sampler from the pipeline's context. Use
    ///   [`TextureView2D::validate_sampler`] with the default view to check the pairing.
    ///
    /// [`Self::build`] checks this binding against the shader and the other resources.
    ///
    /// # Example
    ///
    /// See [`sampled_texture` in the shared example](Resources#example).
    #[must_use]
    pub fn sampled_texture(
        mut self,
        set: u32,
        binding: u32,
        texture: &Texture2D,
        sampler: &Sampler,
    ) -> Self {
        let old = self.bindings.entry(set).or_default().insert(
            binding,
            ResourceBinding::SampledTexture(texture.sampled_binding(sampler)),
        );
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Binds an explicit texture view and sampler to a combined image-sampler descriptor.
    ///
    /// Use [`Texture2D::view`] through [`Self::sampled_texture`] to select the default identity
    /// view. An explicit view keeps the same image storage and upload dependency. The view, sampler,
    /// and pipeline must belong to the same `Context`.
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the combined image-sampler descriptor within that set.
    /// - `view` supplies a texture view from the pipeline's context.
    /// - `sampler` supplies a sampler from the pipeline's context. Use
    ///   [`TextureView2D::validate_sampler`] on `view` to check the pairing.
    ///
    /// [`Self::build`] checks this binding against the shader and the other resources.
    ///
    /// # Example
    ///
    /// See [`sampled_texture_view` in the shared example](Resources#example).
    #[must_use]
    pub fn sampled_texture_view(
        mut self,
        set: u32,
        binding: u32,
        view: &TextureView2D,
        sampler: &Sampler,
    ) -> Self {
        let old = self.bindings.entry(set).or_default().insert(
            binding,
            ResourceBinding::SampledTexture(view.sampled_binding(sampler)),
        );
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Binds a texture's default view to a scalar sampled-image descriptor.
    ///
    /// Pair the image with [`Self::sampler`] at the bindings selected by the shader.
    /// The texture image must include sampled usage. The image and pipeline must belong
    /// to the same `Context`. A pending initial upload is joined into each consuming
    /// frame without adding a host wait.
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the sampled-image descriptor within that set.
    /// - `texture` supplies the default texture view from the pipeline's context.
    ///
    /// [`Self::build`] checks descriptor coverage and image compatibility with the shader.
    ///
    /// # Example
    ///
    /// See [`texture` in the shared example](Resources#example).
    #[must_use]
    pub fn texture(mut self, set: u32, binding: u32, texture: &Texture2D) -> Self {
        let old = self.bindings.entry(set).or_default().insert(
            binding,
            ResourceBinding::SampledImage(texture.sampled_image_binding()),
        );
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Binds an explicit texture view to a scalar sampled-image descriptor.
    ///
    /// Pair the image with [`Self::sampler`] at the bindings selected by the shader. The view's
    /// image must include sampled usage, and the view and pipeline must belong to the same
    /// `Context`. A pending initial upload is joined into each consuming frame without a host wait.
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the sampled-image descriptor within that set.
    /// - `view` supplies a sampled texture view from the pipeline's context.
    ///
    /// [`Self::build`] checks descriptor coverage and view compatibility with the shader.
    ///
    /// # Example
    ///
    /// See [`texture_view` in the shared example](Resources#example).
    #[must_use]
    pub fn texture_view(mut self, set: u32, binding: u32, view: &TextureView2D) -> Self {
        let old = self.bindings.entry(set).or_default().insert(
            binding,
            ResourceBinding::SampledImage(view.sampled_image_binding()),
        );
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Binds a sampler to a scalar sampler descriptor.
    ///
    /// Pair it with [`Self::texture`] according to the shader's actual sampling
    /// instructions. The sampler and pipeline must belong to the same `Context`.
    /// Shader/sampler coordinate-mode compatibility is checked by [`Self::build`].
    ///
    /// # Arguments
    ///
    /// - `set` identifies the descriptor set declared by the shader.
    /// - `binding` identifies the sampler descriptor within that set.
    /// - `sampler` supplies a sampler from [`crate::texture::SamplerBuilder::build`] using
    ///   the pipeline's context and a configuration compatible with the shader's sampling.
    ///
    /// # Example
    ///
    /// See [`sampler` in the shared example](Resources#example).
    #[must_use]
    pub fn sampler(mut self, set: u32, binding: u32, sampler: &Sampler) -> Self {
        let old = self
            .bindings
            .entry(set)
            .or_default()
            .insert(binding, ResourceBinding::Sampler(sampler.sampled_binding()));
        if old.is_some() && self.duplicate_binding.is_none() {
            self.duplicate_binding = Some((set, binding));
        }
        self
    }

    /// Builds descriptor resources compatible with the pipeline used by this builder.
    ///
    /// # Arguments
    ///
    /// - `context` supplies the device and allocator. Pass the context used by the associated
    ///   pipeline.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for missing, extra, unsupported, or duplicate bindings, a context
    /// mismatch, incompatible sampled resources, or disagreeing frame-uniform image counts.
    /// Returns `VulkanDescriptorSetCreationFailed` if a static descriptor set cannot be created.
    /// Bindings for both [`Uniform`] and [`FrameUniform`] are validated here. Descriptor sets
    /// for frame-uniform bindings are allocated during frame recording.
    ///
    /// # Example
    ///
    /// See [`build` in the shared example](Resources#example).
    pub fn build(self, context: &Context) -> VMNLResult<Resources> {
        let Self {
            pipeline_device,
            pipeline_layout,
            set_layouts,
            shader_contract,
            bindings,
            duplicate_binding,
            frame_uniform_image_count,
            incompatible_frame_uniform_image_count,
        } = self;

        validate_resources_context(context, &pipeline_device)?;
        if let Some((set, binding)) = duplicate_binding {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw resources duplicate binding set {set} binding {binding}"
            ))));
        }
        if let Some((expected, actual, set, binding)) = incompatible_frame_uniform_image_count {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw resources frame uniform at set {set} binding {binding} has {actual} images, expected {expected}"
            ))));
        }

        validate_supplied_resource_bindings(
            &pipeline_device,
            &set_layouts,
            &bindings,
            &shader_contract,
        )?;
        let required_set_count = required_descriptor_set_count_from_layouts(&set_layouts);
        let upload_dependencies = collect_upload_dependencies(&bindings);
        let descriptor_sets = match frame_uniform_image_count {
            Some(image_count) => RawDescriptorSets::FrameUniforms {
                image_count,
                set_layouts,
                bindings,
                required_set_count,
            },
            None => RawDescriptorSets::Static {
                sets: build_descriptor_sets(
                    &context.inner.descriptor_set_allocator,
                    &context.inner.device,
                    &set_layouts,
                    &bindings,
                    required_set_count,
                    None,
                )?,
                upload_dependencies,
            },
        };

        Ok(Resources {
            device: context.inner.device.clone(),
            pipeline_layout,
            descriptor_sets,
        })
    }
}

#[derive(Clone)]
enum ResourceBinding {
    FrameUniformBuffer {
        slots: SharedFrameUniformSlots,
        device: Arc<Device>,
        image_count: usize,
    },
    UniformBuffer {
        buffer: Subbuffer<[u8]>,
        device: Arc<Device>,
    },
    SampledTexture(SampledTextureBinding),
    SampledImage(SampledImageBinding),
    Sampler(SampledSamplerBinding),
}

#[derive(Clone)]
enum RawDescriptorSets {
    FrameUniforms {
        image_count: usize,
        set_layouts: Vec<Arc<DescriptorSetLayout>>,
        bindings: BTreeMap<u32, BTreeMap<u32, ResourceBinding>>,
        required_set_count: usize,
    },
    Static {
        sets: Vec<Arc<DescriptorSet>>,
        upload_dependencies: Vec<TextureUploadDependency>,
    },
}

fn collect_upload_dependencies(
    bindings: &BTreeMap<u32, BTreeMap<u32, ResourceBinding>>,
) -> Vec<TextureUploadDependency> {
    bindings
        .values()
        .flat_map(BTreeMap::values)
        .filter_map(|resource| match resource {
            ResourceBinding::SampledTexture(texture) => {
                Some(texture.image.upload_dependency.clone())
            }
            ResourceBinding::SampledImage(image) => Some(image.upload_dependency.clone()),
            ResourceBinding::Sampler(_) => None,
            ResourceBinding::FrameUniformBuffer { .. } | ResourceBinding::UniformBuffer { .. } => {
                None
            }
        })
        .collect()
}

impl RawDescriptorSets {
    fn empty() -> Self {
        Self::Static {
            sets: Vec::new(),
            upload_dependencies: Vec::new(),
        }
    }

    fn upload_dependencies(&self) -> Vec<TextureUploadDependency> {
        match self {
            Self::Static {
                upload_dependencies,
                ..
            } => upload_dependencies.clone(),
            Self::FrameUniforms { bindings, .. } => collect_upload_dependencies(bindings),
        }
    }

    fn for_image(
        &self,
        image_index: usize,
        descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>,
        expected_device: &Arc<Device>,
    ) -> VMNLResult<Vec<Arc<DescriptorSet>>> {
        match self {
            Self::Static { sets, .. } => Ok(sets.clone()),
            Self::FrameUniforms {
                set_layouts,
                bindings,
                required_set_count,
                ..
            } => build_descriptor_sets(
                descriptor_set_allocator,
                expected_device,
                set_layouts,
                bindings,
                *required_set_count,
                Some(image_index),
            ),
        }
    }
}

pub(crate) struct PendingFrameUniformWrite<'a> {
    image_count: usize,
    write: Box<dyn FnOnce(usize) -> VMNLResult<()> + 'a>,
}

impl<'a> PendingFrameUniformWrite<'a> {
    pub(crate) fn new<TData>(uniform: &'a mut FrameUniform<TData>, data: TData) -> Self
    where
        TData: BufferContents + 'a,
    {
        let image_count = uniform.image_count();

        Self {
            image_count,
            write: Box::new(move |image_index| uniform.write_slot(image_index, data)),
        }
    }

    /// Checks whether the swapchain still matches the frame-uniform slot count.
    ///
    /// Call before acquiring an image so a slot-count mismatch can be reported before submission.
    pub(crate) fn validate_image_count(&self, current_image_count: usize) -> VMNLResult<()> {
        validate_frame_uniform_image_count(self.image_count, current_image_count)
    }

    pub(crate) fn write(self, image_index: usize) -> VMNLResult<()> {
        (self.write)(image_index)
    }
}

#[derive(Clone)]
pub(crate) struct RenderItemRaw {
    pub(crate) pipeline: Arc<GraphicsPipeline>,
    pub(crate) pipeline_device: Arc<Device>,
    pub(crate) pipeline_render_pass: Arc<RenderPass>,
    pub(crate) viewport: ViewportPolicy,
    pub(crate) scissor: ScissorPolicy,
    pub(crate) geometry_device: Arc<Device>,
    pub(crate) resources_device: Option<Arc<Device>>,
    pub(crate) resources_pipeline_layout: Option<Arc<PipelineLayout>>,
    descriptor_sets: RawDescriptorSets,
    pub(crate) required_descriptor_set_count: usize,
    pub(crate) vertex_buffer: Subbuffer<[u8]>,
    pub(crate) index_buffer: Option<Subbuffer<[u32]>>,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
}

impl RenderItemRaw {
    /// Collects initial texture uploads that the draw can depend on.
    pub(crate) fn upload_dependencies(&self) -> Vec<TextureUploadDependency> {
        self.descriptor_sets.upload_dependencies()
    }

    /// Checks frame-uniform slot counts and descriptor-set coverage before image acquisition.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for incompatible image counts or incomplete descriptor resources.
    pub(crate) fn validate_descriptor_sets_for_frame(
        &self,
        current_image_count: usize,
    ) -> VMNLResult<()> {
        let set_count = match &self.descriptor_sets {
            RawDescriptorSets::Static { sets, .. } => sets.len(),
            RawDescriptorSets::FrameUniforms {
                image_count,
                required_set_count,
                ..
            } => {
                validate_frame_uniform_image_count(*image_count, current_image_count)?;
                *required_set_count
            }
        };
        if set_count != self.required_descriptor_set_count {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "raw pipeline descriptor resources are incomplete".into(),
            )));
        }
        Ok(())
    }

    /// Resolves the descriptor sets for the acquired image.
    ///
    /// Static sets are reused. Frame-uniform bindings select the slot for the supplied image index
    /// after its pending writes are applied.
    pub(crate) fn descriptor_sets_for_image(
        &self,
        image_index: usize,
        descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>,
        expected_device: &Arc<Device>,
    ) -> VMNLResult<Vec<Arc<DescriptorSet>>> {
        self.descriptor_sets
            .for_image(image_index, descriptor_set_allocator, expected_device)
    }
}

fn validate_geometry_inputs(vertex_count: usize, indices: Option<&[u32]>) -> VMNLResult<()> {
    if vertex_count == 0 {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
            "raw geometry requires at least one vertex".into(),
        )));
    }

    if let Some(indices) = indices {
        if indices.is_empty() {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "raw geometry indices cannot be empty".into(),
            )));
        }

        let vertex_count = u32::try_from(vertex_count).map_err(|_| {
            VMNLError::new(VMNLErrorKind::InvalidState(
                "raw geometry vertex count out of bounds".into(),
            ))
        })?;
        if indices.iter().any(|&index| index >= vertex_count) {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                "raw geometry index out of bounds".into(),
            )));
        }
    }

    Ok(())
}

fn validate_resources_context(context: &Context, expected_device: &Arc<Device>) -> VMNLResult<()> {
    if !Arc::ptr_eq(&context.inner.device, expected_device) {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
            "raw resources must be built from the same context as the raw pipeline".into(),
        )));
    }

    Ok(())
}

fn allocate_frame_uniform_buffer<TData>(
    allocator: &FrameUniformAllocator,
    data: TData,
) -> VMNLResult<Subbuffer<TData>>
where
    TData: BufferContents,
{
    let buffer = allocator
        .allocate_sized()
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanFrameUboBufferCreationFailed))?;

    {
        let mut guard = buffer
            .write()
            .map_err(|error| map_frame_uniform_write_error(&error))?;
        *guard = data;
    }

    Ok(buffer)
}

fn lock_frame_uniform_allocator(
    allocator: &Arc<Mutex<FrameUniformAllocator>>,
) -> VMNLResult<MutexGuard<'_, FrameUniformAllocator>> {
    allocator.lock().map_err(|_| {
        VMNLError::new(VMNLErrorKind::InvalidState(
            "raw frame uniform allocator lock is poisoned".into(),
        ))
    })
}

fn lock_frame_uniform_slots(
    slots: &SharedFrameUniformSlots,
) -> VMNLResult<MutexGuard<'_, FrameUniformSlots>> {
    slots.lock().map_err(|_| {
        VMNLError::new(VMNLErrorKind::InvalidState(
            "raw frame uniform resource state lock is poisoned".into(),
        ))
    })
}

fn map_uniform_write_error(error: &HostAccessError) -> VMNLError {
    map_host_access_write_error(
        error,
        "raw uniform write conflicts with active CPU or GPU access",
    )
}

fn map_frame_uniform_write_error(error: &HostAccessError) -> VMNLError {
    map_host_access_write_error(
        error,
        "raw frame uniform write conflicts with active CPU or GPU access",
    )
}

fn map_host_access_write_error(error: &HostAccessError, conflict_message: &str) -> VMNLError {
    match error {
        HostAccessError::AccessConflict(_) => {
            VMNLError::new(VMNLErrorKind::InvalidState(conflict_message.into()))
        }
        _ => VMNLError::new(VMNLErrorKind::VulkanValidationFailed),
    }
}

fn validate_frame_uniform_image_count(
    expected_image_count: usize,
    current_image_count: usize,
) -> VMNLResult<()> {
    if expected_image_count != current_image_count {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
            FRAME_UNIFORM_IMAGE_COUNT_MISMATCH.into(),
        )));
    }

    Ok(())
}

fn next_frame_uniform_image_count_mismatch(
    expected_image_count: &mut Option<usize>,
    actual_image_count: usize,
    set: u32,
    binding: u32,
) -> Option<(usize, usize, u32, u32)> {
    match *expected_image_count {
        Some(expected) if expected != actual_image_count => {
            Some((expected, actual_image_count, set, binding))
        }
        Some(_) => None,
        None => {
            *expected_image_count = Some(actual_image_count);
            None
        }
    }
}

fn validate_supplied_resource_bindings(
    expected_device: &Arc<Device>,
    set_layouts: &[Arc<DescriptorSetLayout>],
    supplied_sets: &BTreeMap<u32, BTreeMap<u32, ResourceBinding>>,
    shader_contract: &ShaderContract,
) -> VMNLResult<()> {
    for (&set, supplied_bindings) in supplied_sets {
        let set_layout = set_layouts.get(usize::try_from(set).map_err(|_| {
            VMNLError::new(VMNLErrorKind::InvalidState(
                "raw resources set index out of bounds".into(),
            ))
        })?);
        let Some(set_layout) = set_layout else {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw resources set {set} is not declared by the pipeline"
            ))));
        };

        for &binding in supplied_bindings.keys() {
            if !set_layout.bindings().contains_key(&binding) {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw resources set {set} binding {binding} is not declared by the pipeline"
                ))));
            }
        }
    }

    for (set_index, set_layout) in set_layouts.iter().enumerate() {
        let set = u32::try_from(set_index)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        for (&binding, binding_layout) in set_layout.bindings() {
            validate_raw_descriptor_binding(set, binding, binding_layout.descriptor_type)?;
            if binding_layout.descriptor_count != 1 {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw resources set {set} binding {binding} descriptor arrays are not supported yet"
                ))));
            }

            let resource = supplied_sets
                .get(&set)
                .and_then(|bindings| bindings.get(&binding))
                .ok_or_else(|| {
                    VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources missing set {set} binding {binding}"
                    )))
                })?;
            let device = match resource {
                ResourceBinding::FrameUniformBuffer { device, .. }
                | ResourceBinding::UniformBuffer { device, .. } => device,
                ResourceBinding::SampledTexture(texture) => &texture.image.device,
                ResourceBinding::SampledImage(image) => &image.device,
                ResourceBinding::Sampler(sampler) => &sampler.device,
            };
            if !Arc::ptr_eq(device, expected_device) {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw resources set {set} binding {binding} must belong to this context"
                ))));
            }

            let resource_type = match resource {
                ResourceBinding::FrameUniformBuffer { .. }
                | ResourceBinding::UniformBuffer { .. } => DescriptorType::UniformBuffer,
                ResourceBinding::SampledTexture(_) => DescriptorType::CombinedImageSampler,
                ResourceBinding::SampledImage(_) => DescriptorType::SampledImage,
                ResourceBinding::Sampler(_) => DescriptorType::Sampler,
            };
            if binding_layout.descriptor_type != resource_type {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw resources set {set} binding {binding} resource does not match the pipeline descriptor type"
                ))));
            }

            if let ResourceBinding::SampledTexture(texture) = resource {
                if let Some(reason) = &texture.validation_error {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding}: {reason}"
                    ))));
                }
                validate_sampled_image_binding(
                    BindingPoint { set, binding },
                    &texture.image,
                    shader_contract,
                )?;
                validate_sampler_shader_binding(
                    BindingPoint { set, binding },
                    &texture.sampler,
                    shader_contract,
                )?;
            }
            if let ResourceBinding::SampledImage(image) = resource {
                validate_sampled_image_binding(
                    BindingPoint { set, binding },
                    image,
                    shader_contract,
                )?;
            }
            if let ResourceBinding::Sampler(sampler) = resource {
                validate_sampler_shader_binding(
                    BindingPoint { set, binding },
                    sampler,
                    shader_contract,
                )?;
            }
        }
    }

    validate_sampled_pairs(supplied_sets, shader_contract)?;

    Ok(())
}

fn validate_sampled_image_binding(
    point: BindingPoint,
    image: &SampledImageBinding,
    shader_contract: &ShaderContract,
) -> VMNLResult<()> {
    if !image.sampled_usage {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} image usage does not include shader sampling",
            point.set, point.binding
        ))));
    }

    let Some(requirements) = shader_contract.descriptors.get(&point) else {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} has no reflected shader requirements",
            point.set, point.binding
        ))));
    };
    if requirements
        .image_view_type
        .is_some_and(|view_type| view_type != vulkano::image::view::ImageViewType::Dim2d)
    {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} requires an image view type incompatible with Texture2D",
            point.set, point.binding
        ))));
    }
    if requirements.image_multisampled {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} requires a multisampled image",
            point.set, point.binding
        ))));
    }
    if requirements
        .image_scalar_type
        .is_some_and(|scalar_type| scalar_type != NumericType::Float)
    {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} shader scalar type is incompatible with normalized RGBA8 texture data",
            point.set, point.binding
        ))));
    }
    if requirements
        .image_format
        .is_some_and(|format| image.image_view.format() != format)
    {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} shader image format is incompatible with the texture view",
            point.set, point.binding
        ))));
    }

    Ok(())
}

fn validate_sampler_shader_binding(
    point: BindingPoint,
    sampler: &SampledSamplerBinding,
    shader_contract: &ShaderContract,
) -> VMNLResult<()> {
    let requirements = shader_contract.descriptors.get(&point).ok_or_else(|| {
        VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} has no reflected shader requirements",
            point.set, point.binding
        )))
    })?;

    let sampler_uses = shader_contract
        .sampler_uses
        .iter()
        .filter(|usage| usage.pair.sampler == point);

    // Vulkano 0.35.2 does not flag depth gather as a comparison.
    if sampler_uses
        .clone()
        .any(|usage| usage.operation == SamplerOperation::DepthComparison)
        || requirements
            .descriptors
            .values()
            .any(|descriptor| descriptor.sampler_compare)
    {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {} binding {} requires depth comparison, which VMNL samplers do not expose",
            point.set, point.binding
        ))));
    }

    if sampler.config.coordinate_mode() == crate::texture::SamplerCoordinateMode::Texel {
        if requirements
            .descriptors
            .values()
            .any(|descriptor| descriptor.sampler_no_unnormalized_coordinates)
        {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw resources set {} binding {} shader sampling instructions are incompatible with texel coordinates",
                point.set, point.binding
            ))));
        }
        for usage in sampler_uses {
            if let SamplerOperation::TexelIncompatible(reason) = usage.operation {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw resources sampler set {} binding {}: {reason}",
                    point.set, point.binding
                ))));
            }
        }
    }

    Ok(())
}

fn validate_sampled_pairs(
    supplied_sets: &BTreeMap<u32, BTreeMap<u32, ResourceBinding>>,
    shader_contract: &ShaderContract,
) -> VMNLResult<()> {
    for pair in &shader_contract.sampled_pairs {
        let image_resource = supplied_sets
            .get(&pair.image.set)
            .and_then(|bindings| bindings.get(&pair.image.binding));
        let sampler_resource = supplied_sets
            .get(&pair.sampler.set)
            .and_then(|bindings| bindings.get(&pair.sampler.binding));

        let (image, sampler) = match (image_resource, sampler_resource) {
            (
                Some(ResourceBinding::SampledTexture(texture)),
                Some(ResourceBinding::SampledTexture(_)),
            ) if pair.image == pair.sampler => (&texture.image, &texture.sampler),
            (
                Some(ResourceBinding::SampledImage(image)),
                Some(ResourceBinding::Sampler(sampler)),
            ) => (image, sampler),
            _ => {
                return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw shader sampled image set {} binding {} with sampler set {} binding {}, but the resource bindings do not form that pair",
                    pair.image.set,
                    pair.image.binding,
                    pair.sampler.set,
                    pair.sampler.binding
                ))));
            }
        };

        validate_sampled_image_sampler(
            &image.image_view,
            &image.device,
            image.sampled_usage,
            &sampler.device,
            sampler.config,
        )
        .map_err(|error| {
            VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw sampled image set {} binding {} and sampler set {} binding {}: {}",
                pair.image.set, pair.image.binding, pair.sampler.set, pair.sampler.binding, error
            )))
        })?;
    }

    Ok(())
}

fn descriptor_writes_for_set(
    expected_device: &Arc<Device>,
    set: u32,
    set_layout: &Arc<DescriptorSetLayout>,
    supplied_bindings: Option<&BTreeMap<u32, ResourceBinding>>,
    frame_image_index: Option<usize>,
) -> VMNLResult<Vec<WriteDescriptorSet>> {
    let mut writes = Vec::with_capacity(supplied_bindings.map_or(0, BTreeMap::len));

    for (&binding, binding_layout) in set_layout.bindings() {
        validate_raw_descriptor_binding(set, binding, binding_layout.descriptor_type)?;
        if binding_layout.descriptor_count != 1 {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw resources set {set} binding {binding} descriptor arrays are not supported yet"
            ))));
        }

        let resource = supplied_bindings
            .and_then(|bindings| bindings.get(&binding))
            .ok_or_else(|| {
                VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "raw resources missing set {set} binding {binding}"
                )))
            })?;

        match resource {
            ResourceBinding::FrameUniformBuffer {
                slots,
                device,
                image_count,
            } => {
                if !Arc::ptr_eq(device, expected_device) {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding} must belong to this context"
                    ))));
                }

                let image_index =
                    descriptor_frame_image_index(frame_image_index, *image_count, set, binding)?;
                let buffer = frame_uniform_binding_buffer(slots, image_index, set, binding)?;
                writes.push(WriteDescriptorSet::buffer(binding, buffer));
            }
            ResourceBinding::UniformBuffer { buffer, device } => {
                if !Arc::ptr_eq(device, expected_device) {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding} must belong to this context"
                    ))));
                }
                writes.push(WriteDescriptorSet::buffer(binding, buffer.clone()));
            }
            ResourceBinding::SampledTexture(texture) => {
                if !Arc::ptr_eq(&texture.image.device, expected_device)
                    || !Arc::ptr_eq(&texture.sampler.device, expected_device)
                {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding} must belong to this context"
                    ))));
                }
                if let Some(reason) = &texture.validation_error {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding}: {reason}"
                    ))));
                }
                writes.push(WriteDescriptorSet::image_view_with_layout_sampler(
                    binding,
                    DescriptorImageViewInfo {
                        image_view: texture.image.image_view.clone(),
                        image_layout: ImageLayout::General,
                    },
                    texture.sampler.sampler.clone(),
                ));
            }
            ResourceBinding::SampledImage(image) => {
                if !Arc::ptr_eq(&image.device, expected_device) {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding} must belong to this context"
                    ))));
                }
                if !image.sampled_usage {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding} image usage does not include shader sampling"
                    ))));
                }
                writes.push(WriteDescriptorSet::image_view_with_layout(
                    binding,
                    DescriptorImageViewInfo {
                        image_view: image.image_view.clone(),
                        image_layout: ImageLayout::General,
                    },
                ));
            }
            ResourceBinding::Sampler(sampler) => {
                if !Arc::ptr_eq(&sampler.device, expected_device) {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                        "raw resources set {set} binding {binding} must belong to this context"
                    ))));
                }
                writes.push(WriteDescriptorSet::sampler(
                    binding,
                    sampler.sampler.clone(),
                ));
            }
        }
    }

    Ok(writes)
}

fn frame_uniform_binding_buffer(
    slots: &SharedFrameUniformSlots,
    image_index: usize,
    set: u32,
    binding: u32,
) -> VMNLResult<Subbuffer<[u8]>> {
    let slots = lock_frame_uniform_slots(slots)?;
    slots.buffers.get(image_index).cloned().ok_or_else(|| {
        VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {set} binding {binding} frame uniform image index is out of bounds"
        )))
    })
}

fn descriptor_frame_image_index(
    frame_image_index: Option<usize>,
    image_count: usize,
    set: u32,
    binding: u32,
) -> VMNLResult<usize> {
    let Some(image_index) = frame_image_index else {
        return Err(VMNLError::new(VMNLErrorKind::VulkanValidationFailed));
    };

    if image_index >= image_count {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
            "raw resources set {set} binding {binding} frame uniform image index is out of bounds"
        ))));
    }

    Ok(image_index)
}

fn build_descriptor_sets(
    descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>,
    expected_device: &Arc<Device>,
    set_layouts: &[Arc<DescriptorSetLayout>],
    supplied_sets: &BTreeMap<u32, BTreeMap<u32, ResourceBinding>>,
    required_set_count: usize,
    frame_image_index: Option<usize>,
) -> VMNLResult<Vec<Arc<DescriptorSet>>> {
    let mut descriptor_sets = Vec::with_capacity(required_set_count);

    for set_index in 0..required_set_count {
        let set = u32::try_from(set_index)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        let set_layout = set_layouts
            .get(set_index)
            .cloned()
            .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        let writes = descriptor_writes_for_set(
            expected_device,
            set,
            &set_layout,
            supplied_sets.get(&set),
            frame_image_index,
        )?;
        let descriptor_set = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            set_layout,
            writes,
            Vec::new(),
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanDescriptorSetCreationFailed))?;
        descriptor_sets.push(descriptor_set);
    }

    Ok(descriptor_sets)
}

fn validate_raw_pipeline_layout(
    layout_info: &PipelineDescriptorSetLayoutCreateInfo,
) -> VMNLResult<()> {
    if !layout_info.push_constant_ranges.is_empty() {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
            "raw pipeline push constants are not supported yet".into(),
        )));
    }

    for (set, set_layout) in layout_info.set_layouts.iter().enumerate() {
        let set = u32::try_from(set)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        validate_raw_descriptor_set_layout(set, set_layout)?;
    }

    Ok(())
}

fn validate_raw_descriptor_set_layout(
    set: u32,
    set_layout: &DescriptorSetLayoutCreateInfo,
) -> VMNLResult<()> {
    for (&binding, binding_layout) in &set_layout.bindings {
        validate_raw_descriptor_binding(set, binding, binding_layout.descriptor_type)?;
        if binding_layout.descriptor_count != 1 {
            return Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "raw pipeline set {set} binding {binding} descriptor arrays are not supported yet"
            ))));
        }
    }

    Ok(())
}

fn validate_raw_descriptor_binding(
    set: u32,
    binding: u32,
    descriptor_type: DescriptorType,
) -> VMNLResult<()> {
    shader_compat::validate_raw_descriptor_type(set, binding, descriptor_type)
}

fn required_descriptor_set_count(layout: &PipelineLayout) -> usize {
    required_descriptor_set_count_from_layouts(layout.set_layouts())
}

fn required_descriptor_set_count_from_layouts(set_layouts: &[Arc<DescriptorSetLayout>]) -> usize {
    set_layouts
        .iter()
        .rposition(|set_layout| !set_layout.bindings().is_empty())
        .map_or(0, |index| index + 1)
}

struct CompiledRawShader {
    module: Arc<ShaderModule>,
    contract: ShaderStageContract,
}

fn compile_raw_shader_stages(
    device: Arc<Device>,
    vertex_source: &ShaderSource,
    fragment_source: &ShaderSource,
) -> VMNLResult<(EntryPoint, EntryPoint, Arc<ShaderContract>)> {
    let vertex_shader = compile_shader(
        device.clone(),
        vertex_source,
        shaderc::ShaderKind::Vertex,
        vulkano::shader::spirv::ExecutionModel::Vertex,
        "vertex",
    )?;
    let fragment_shader = compile_shader(
        device,
        fragment_source,
        shaderc::ShaderKind::Fragment,
        vulkano::shader::spirv::ExecutionModel::Fragment,
        "fragment",
    )?;
    let contract = Arc::new(ShaderContract::merge(
        &vertex_shader.contract,
        &fragment_shader.contract,
    )?);
    let vertex_entry = vertex_shader
        .module
        .entry_point("main")
        .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanShaderCompilationFailed))?;
    let fragment_entry = fragment_shader
        .module
        .entry_point("main")
        .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanShaderCompilationFailed))?;

    Ok((vertex_entry, fragment_entry, contract))
}

fn compile_shader(
    device: Arc<Device>,
    source: &ShaderSource,
    kind: shaderc::ShaderKind,
    execution_model: vulkano::shader::spirv::ExecutionModel,
    stage_name: &str,
) -> VMNLResult<CompiledRawShader> {
    let (source, name) = match source {
        ShaderSource::Src(source) => (source.clone(), "raw_inline_shader.glsl".to_string()),
        ShaderSource::Path(path) => {
            let source = std::fs::read_to_string(path).map_err(|err| {
                VMNLError::new(VMNLErrorKind::InvalidState(format!(
                    "failed to read shader '{}': {}",
                    path.display(),
                    err
                )))
            })?;
            (source, path.display().to_string())
        }
    };

    let compiler = shaderc::Compiler::new()
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanShaderCompilationFailed))?;

    let spirv = compiler
        .compile_into_spirv(&source, kind, &name, "main", None)
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanShaderCompilationFailed))?;

    let words = spirv.as_binary().to_vec();
    let create_info = ShaderModuleCreateInfo::new(&words);
    // SAFETY: shaderc returns valid SPIR-V for the requested stage or an error.
    let module = unsafe { ShaderModule::new(device, create_info) }
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanShaderModuleCreationFailed))?;
    let contract = shader_compat::reflect_stage_contract(&words, execution_model, stage_name)?;
    Ok(CompiledRawShader { module, contract })
}

fn validate_raw_rasterization(
    polygon_mode: PolygonMode,
    line_width: f32,
    limits: LineWidthLimits,
    enabled_features: &DeviceFeatures,
    is_portability_subset: bool,
) -> VMNLResult<()> {
    if !line_width.is_finite()
        || line_width <= 0.0
        || line_width < limits.min
        || line_width > limits.max
    {
        return Err(VMNLError::new(VMNLErrorKind::InvalidLineWidth {
            value: line_width,
            min: limits.min,
            max: limits.max,
        }));
    }

    if polygon_mode != PolygonMode::Fill && !enabled_features.fill_mode_non_solid {
        return Err(VMNLError::new(VMNLErrorKind::DeviceFeatureNotEnabled {
            feature: DeviceFeature::FillModeNonSolid,
        }));
    }

    if polygon_mode == PolygonMode::Point
        && is_portability_subset
        && !enabled_features.point_polygons
    {
        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
            "raw point polygon mode requires pointPolygons on portability-subset devices; DeviceConfig cannot currently request it".into(),
        )));
    }

    // Vulkan requires exactly 1.0 without wideLines; an epsilon would accept invalid widths.
    if line_width.to_bits() != 1.0_f32.to_bits() && !enabled_features.wide_lines {
        return Err(VMNLError::new(VMNLErrorKind::DeviceFeatureNotEnabled {
            feature: DeviceFeature::WideLines,
        }));
    }

    Ok(())
}

fn raw_rasterization_state(
    cull_mode: CullMode,
    front_face: FrontFace,
    polygon_mode: PolygonMode,
    line_width: f32,
) -> RasterizationState {
    RasterizationState {
        cull_mode: match cull_mode {
            CullMode::None => VulkanoCullMode::None,
            CullMode::Front => VulkanoCullMode::Front,
            CullMode::Back => VulkanoCullMode::Back,
            CullMode::FrontAndBack => VulkanoCullMode::FrontAndBack,
        },
        front_face: match front_face {
            FrontFace::CounterClockwise => VulkanoFrontFace::CounterClockwise,
            FrontFace::Clockwise => VulkanoFrontFace::Clockwise,
        },
        polygon_mode: match polygon_mode {
            PolygonMode::Fill => VulkanoPolygonMode::Fill,
            PolygonMode::Line => VulkanoPolygonMode::Line,
            PolygonMode::Point => VulkanoPolygonMode::Point,
        },
        line_width,
        ..Default::default()
    }
}

fn color_blend_state(config: ColorBlendConfig) -> ColorBlendState {
    let blend = config.blend_enabled().then_some(AttachmentBlend {
        src_color_blend_factor: config.color_source_factor().to_vulkano(),
        dst_color_blend_factor: config.color_destination_factor().to_vulkano(),
        color_blend_op: config.color_operation().to_vulkano(),
        src_alpha_blend_factor: config.alpha_source_factor().to_vulkano(),
        dst_alpha_blend_factor: config.alpha_destination_factor().to_vulkano(),
        alpha_blend_op: config.alpha_operation().to_vulkano(),
    });
    let mut state = ColorBlendState::with_attachment_states(
        1,
        ColorBlendAttachmentState {
            blend,
            ..Default::default()
        },
    );
    state.blend_constants = config.blend_constants();
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use vulkano::descriptor_set::layout::DescriptorSetLayoutBinding;
    use vulkano::pipeline::layout::{PipelineLayoutCreateFlags, PushConstantRange};
    use vulkano::shader::ShaderStages;
    use vulkano::sync::AccessConflict;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct TestVertex {
        position: [f32; 2],
    }

    #[test]
    fn pipeline_spec_defaults_are_triangle_list_and_opaque() {
        let spec = PipelineSpec::<TestVertex>::default();

        assert_eq!(spec.topology_value(), PrimitiveTopology::TriangleList);
        assert_eq!(spec.blend_mode_value(), BlendMode::Opaque);
    }

    #[test]
    fn raw_rasterization_preserves_face_selection_and_other_defaults() {
        for (cull_mode, expected_cull_mode) in [
            (CullMode::None, VulkanoCullMode::None),
            (CullMode::Front, VulkanoCullMode::Front),
            (CullMode::Back, VulkanoCullMode::Back),
            (CullMode::FrontAndBack, VulkanoCullMode::FrontAndBack),
        ] {
            for (front_face, expected_front_face) in [
                (
                    FrontFace::CounterClockwise,
                    VulkanoFrontFace::CounterClockwise,
                ),
                (FrontFace::Clockwise, VulkanoFrontFace::Clockwise),
            ] {
                let state = raw_rasterization_state(cull_mode, front_face, PolygonMode::Fill, 1.0);
                assert_eq!(state.cull_mode, expected_cull_mode);
                assert_eq!(state.front_face, expected_front_face);
                assert_eq!(
                    state.polygon_mode,
                    vulkano::pipeline::graphics::rasterization::PolygonMode::Fill
                );
                assert_eq!(state.line_width.to_bits(), 1.0_f32.to_bits());
                assert!(!state.depth_clamp_enable);
                assert!(!state.rasterizer_discard_enable);
                assert!(state.depth_bias.is_none());
            }
        }
    }

    fn test_line_width_limits() -> LineWidthLimits {
        LineWidthLimits {
            min: 0.5,
            max: 4.0,
            granularity: 0.25,
        }
    }

    #[test]
    fn raw_rasterization_maps_polygon_modes_and_line_width_without_changing_other_state() {
        for (mode, expected) in [
            (PolygonMode::Fill, VulkanoPolygonMode::Fill),
            (PolygonMode::Line, VulkanoPolygonMode::Line),
            (PolygonMode::Point, VulkanoPolygonMode::Point),
        ] {
            for width in [0.5_f32, 1.0, 2.5, 4.0] {
                let state =
                    raw_rasterization_state(CullMode::Back, FrontFace::Clockwise, mode, width);
                assert_eq!(state.polygon_mode, expected);
                assert_eq!(state.line_width.to_bits(), width.to_bits());
                assert_eq!(state.cull_mode, VulkanoCullMode::Back);
                assert_eq!(state.front_face, VulkanoFrontFace::Clockwise);
                assert!(!state.depth_clamp_enable);
                assert!(!state.rasterizer_discard_enable);
                assert!(state.depth_bias.is_none());
            }
        }
    }

    #[test]
    fn raw_rasterization_rejects_invalid_widths_before_checking_features() {
        for width in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            -0.0,
            0.0,
            0.49,
            4.01,
        ] {
            let result = validate_raw_rasterization(
                PolygonMode::Line,
                width,
                test_line_width_limits(),
                &DeviceFeatures::empty(),
                false,
            );
            assert!(matches!(result, Err(error) if matches!(error.kind(),
                VMNLErrorKind::InvalidLineWidth { value, min, max }
                if value.to_bits() == width.to_bits()
                    && min.to_bits() == 0.5_f32.to_bits() && max.to_bits() == 4.0_f32.to_bits()
            )));
        }
    }

    #[test]
    fn raw_rasterization_checks_activated_features_including_subunit_widths() {
        let solid = DeviceFeatures::empty();
        let non_solid = DeviceFeatures {
            fill_mode_non_solid: true,
            ..DeviceFeatures::empty()
        };
        let wide = DeviceFeatures {
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        let both = DeviceFeatures {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        for (mode, width, features, missing) in [
            (PolygonMode::Fill, 1.0, &solid, None),
            (
                PolygonMode::Line,
                1.0,
                &solid,
                Some(DeviceFeature::FillModeNonSolid),
            ),
            (
                PolygonMode::Point,
                1.0,
                &solid,
                Some(DeviceFeature::FillModeNonSolid),
            ),
            (PolygonMode::Line, 1.0, &non_solid, None),
            (PolygonMode::Point, 1.0, &non_solid, None),
            (
                PolygonMode::Fill,
                0.5,
                &solid,
                Some(DeviceFeature::WideLines),
            ),
            (
                PolygonMode::Fill,
                4.0,
                &solid,
                Some(DeviceFeature::WideLines),
            ),
            (PolygonMode::Fill, 0.5, &wide, None),
            (PolygonMode::Fill, 4.0, &wide, None),
            (
                PolygonMode::Line,
                2.5,
                &non_solid,
                Some(DeviceFeature::WideLines),
            ),
            (
                PolygonMode::Line,
                2.5,
                &wide,
                Some(DeviceFeature::FillModeNonSolid),
            ),
            (PolygonMode::Line, 2.5, &both, None),
            (PolygonMode::Point, 2.5, &both, None),
        ] {
            let result =
                validate_raw_rasterization(mode, width, test_line_width_limits(), features, false);
            match missing {
                None => assert!(result.is_ok(), "{mode:?}, width {width}: {result:?}"),
                Some(expected) => assert!(matches!(result, Err(error) if matches!(error.kind(),
                    VMNLErrorKind::DeviceFeatureNotEnabled { feature } if *feature == expected))),
            }
        }
        assert!(validate_raw_rasterization(
            PolygonMode::Fill,
            1.0,
            LineWidthLimits {
                min: 1.0,
                max: 1.0,
                granularity: 0.0
            },
            &solid,
            false
        )
        .is_ok());
        let zero_min = LineWidthLimits {
            min: 0.0,
            max: 8.0,
            granularity: 0.007_812_5,
        };
        assert!(validate_raw_rasterization(PolygonMode::Fill, 0.5, zero_min, &wide, false).is_ok());
        assert!(
            matches!(validate_raw_rasterization(PolygonMode::Fill, 0.0, zero_min, &wide, false),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidLineWidth { .. }))
        );
    }

    #[test]
    fn raw_rasterization_requires_wide_lines_for_float_neighbors_of_one() {
        for bits in [1.0_f32.to_bits() - 1, 1.0_f32.to_bits() + 1] {
            assert!(matches!(validate_raw_rasterization(PolygonMode::Fill,
            f32::from_bits(bits), test_line_width_limits(), &DeviceFeatures::empty(), false),
            Err(error) if matches!(error.kind(), VMNLErrorKind::DeviceFeatureNotEnabled {
                feature: DeviceFeature::WideLines,
            })));
        }
    }

    #[test]
    fn raw_point_polygons_require_portability_activation_only_on_subset_devices() {
        for (is_subset, point_polygons, mode, succeeds) in [
            (false, false, PolygonMode::Point, true),
            (true, false, PolygonMode::Point, false),
            (true, true, PolygonMode::Point, true),
            (true, false, PolygonMode::Line, true),
        ] {
            let features = DeviceFeatures {
                fill_mode_non_solid: true,
                point_polygons,
                ..DeviceFeatures::empty()
            };
            let result = validate_raw_rasterization(
                mode,
                1.0,
                test_line_width_limits(),
                &features,
                is_subset,
            );
            if succeeds {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(error) if matches!(error.kind(),
                    VMNLErrorKind::InvalidState(message) if message.contains("pointPolygons"))));
            }
        }
    }

    #[test]
    fn geometry_builder_refuses_empty_vertices() {
        let result = validate_geometry_inputs(0, None);

        assert!(result.is_err());
    }

    #[test]
    fn geometry_builder_refuses_index_out_of_bounds() {
        let result = validate_geometry_inputs(3, Some(&[0, 3]));

        assert!(result.is_err());
    }

    #[test]
    fn geometry_builder_accepts_indexed_and_non_indexed() {
        assert!(validate_geometry_inputs(3, None).is_ok());
        assert!(validate_geometry_inputs(3, Some(&[0, 1, 2])).is_ok());
    }

    #[test]
    fn raw_pipeline_accepts_uniform_descriptor_layouts() {
        let layout_info = pipeline_layout_info(vec![descriptor_set_layout(
            DescriptorType::UniformBuffer,
            1,
        )]);

        assert!(validate_raw_pipeline_layout(&layout_info).is_ok());
    }

    #[test]
    fn raw_pipeline_accepts_combined_image_sampler_descriptor_layouts() {
        let layout_info = pipeline_layout_info(vec![descriptor_set_layout(
            DescriptorType::CombinedImageSampler,
            1,
        )]);

        assert!(validate_raw_pipeline_layout(&layout_info).is_ok());
    }

    #[test]
    fn raw_pipeline_refuses_unsupported_descriptor_layouts() {
        let layout_info = pipeline_layout_info(vec![descriptor_set_layout(
            DescriptorType::StorageBuffer,
            1,
        )]);

        assert!(validate_raw_pipeline_layout(&layout_info).is_err());
    }

    #[test]
    fn raw_pipeline_refuses_descriptor_arrays() {
        let layout_info = pipeline_layout_info(vec![descriptor_set_layout(
            DescriptorType::UniformBuffer,
            2,
        )]);

        assert!(validate_raw_pipeline_layout(&layout_info).is_err());
    }

    #[test]
    fn raw_pipeline_refuses_push_constants() {
        let layout_info = PipelineDescriptorSetLayoutCreateInfo {
            flags: PipelineLayoutCreateFlags::empty(),
            set_layouts: Vec::new(),
            push_constant_ranges: vec![PushConstantRange {
                stages: ShaderStages::VERTEX,
                offset: 0,
                size: 4,
            }],
        };

        assert!(validate_raw_pipeline_layout(&layout_info).is_err());
    }

    #[test]
    fn uniform_write_conflict_maps_to_invalid_state() {
        let error =
            map_uniform_write_error(&HostAccessError::AccessConflict(AccessConflict::DeviceRead));

        assert!(matches!(
            error.kind(),
            VMNLErrorKind::InvalidState(message)
                if message == "raw uniform write conflicts with active CPU or GPU access"
        ));
    }

    #[test]
    fn uniform_write_backend_error_maps_to_vulkan_validation() {
        let error = map_uniform_write_error(&HostAccessError::NotHostMapped);

        assert!(matches!(
            error.kind(),
            VMNLErrorKind::VulkanValidationFailed
        ));
    }

    #[test]
    fn frame_uniform_write_conflict_maps_to_invalid_state() {
        let error = map_frame_uniform_write_error(&HostAccessError::AccessConflict(
            AccessConflict::DeviceRead,
        ));

        assert!(matches!(
            error.kind(),
            VMNLErrorKind::InvalidState(message)
                if message == "raw frame uniform write conflicts with active CPU or GPU access"
        ));
    }

    #[test]
    fn frame_uniform_write_backend_error_maps_to_vulkan_validation() {
        let error = map_frame_uniform_write_error(&HostAccessError::NotHostMapped);

        assert!(matches!(
            error.kind(),
            VMNLErrorKind::VulkanValidationFailed
        ));
    }

    #[test]
    fn frame_uniform_image_count_mismatch_is_invalid_state() {
        let result = validate_frame_uniform_image_count(2, 3);

        assert!(matches!(
            result,
            Err(error)
                if matches!(
                    error.kind(),
                    VMNLErrorKind::InvalidState(message)
                        if message == FRAME_UNIFORM_IMAGE_COUNT_MISMATCH
                )
        ));
    }

    #[test]
    fn frame_uniform_resource_image_count_mismatch_is_detected() {
        let mut expected_image_count = None;

        assert_eq!(
            next_frame_uniform_image_count_mismatch(&mut expected_image_count, 2, 0, 0),
            None
        );
        assert_eq!(expected_image_count, Some(2));
        assert_eq!(
            next_frame_uniform_image_count_mismatch(&mut expected_image_count, 3, 1, 0),
            Some((2, 3, 1, 0))
        );
    }

    #[test]
    fn raw_descriptor_sets_select_static_and_frame_uniform_modes() {
        let static_sets = RawDescriptorSets::Static {
            sets: Vec::new(),
            upload_dependencies: Vec::new(),
        };
        let frame_uniform_sets = RawDescriptorSets::FrameUniforms {
            image_count: 2,
            set_layouts: Vec::new(),
            bindings: BTreeMap::new(),
            required_set_count: 0,
        };

        assert!(matches!(
            static_sets,
            RawDescriptorSets::Static { ref sets, .. } if sets.is_empty()
        ));
        assert!(matches!(
            frame_uniform_sets,
            RawDescriptorSets::FrameUniforms {
                image_count: 2,
                required_set_count: 0,
                ..
            }
        ));
    }

    fn pipeline_layout_info(
        set_layouts: Vec<DescriptorSetLayoutCreateInfo>,
    ) -> PipelineDescriptorSetLayoutCreateInfo {
        PipelineDescriptorSetLayoutCreateInfo {
            flags: PipelineLayoutCreateFlags::empty(),
            set_layouts,
            push_constant_ranges: Vec::new(),
        }
    }

    fn descriptor_set_layout(
        descriptor_type: DescriptorType,
        descriptor_count: u32,
    ) -> DescriptorSetLayoutCreateInfo {
        let mut layout = DescriptorSetLayoutCreateInfo::default();
        let mut binding = DescriptorSetLayoutBinding::descriptor_type(descriptor_type);
        binding.stages = ShaderStages::VERTEX;
        binding.descriptor_count = descriptor_count;
        layout.bindings.insert(0, binding);
        layout
    }
}
