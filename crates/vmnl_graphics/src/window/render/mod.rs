// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Draw submodule for handling rendering operations in the VMNL application.
//!
//! This module provides functionality to build command buffers, manage frame synchronization,
//! and execute draw calls using Vulkan through the Vulkano library.

mod acquire;
mod command_buffer;
mod sync;

use crate::d2::RenderItem2D;
use crate::raw::{PendingFrameUniformWrite, RenderItemRaw};
use crate::texture::TextureUploadDependency;
use crate::window::{inner::VMNLWindow, RenderMode};
use crate::{VMNLError, VMNLErrorKind, VMNLResult};
use std::sync::Arc;
use vulkano::{
    command_buffer::PrimaryAutoCommandBuffer, swapchain::SwapchainAcquireFuture, sync::GpuFuture,
};

pub(crate) enum RenderPassCommand {
    D2 { items: Vec<RenderItem2D> },
    Raw { items: Vec<RenderItemRaw> },
}

fn raw_draw_items(commands: &[RenderPassCommand]) -> impl Iterator<Item = &RenderItemRaw> {
    commands.iter().flat_map(|command| match command {
        RenderPassCommand::Raw { items } => items.as_slice(),
        RenderPassCommand::D2 { .. } => &[],
    })
}

fn prepare_texture_uploads(
    commands: &[RenderPassCommand],
) -> VMNLResult<Vec<TextureUploadDependency>> {
    let mut dependencies = Vec::new();
    for dependency in raw_draw_items(commands).flat_map(RenderItemRaw::upload_dependencies) {
        if !dependencies
            .iter()
            .any(|seen| Arc::ptr_eq(seen, &dependency))
        {
            dependency.check_for_failure()?;
            dependencies.push(dependency);
        }
    }
    Ok(dependencies)
}

impl VMNLWindow {
    pub(crate) fn render_commands(
        &mut self,
        mode: RenderMode,
        commands: &[RenderPassCommand],
        frame_uniform_writes: Vec<PendingFrameUniformWrite<'_>>,
    ) -> VMNLResult<()> {
        match mode {
            RenderMode::PerObject | RenderMode::Batched => {
                self.render_per_object(commands, frame_uniform_writes)
            }
        }
    }

    fn render_per_object(
        &mut self,
        commands: &[RenderPassCommand],
        frame_uniform_writes: Vec<PendingFrameUniformWrite<'_>>,
    ) -> VMNLResult<()> {
        let upload_dependencies = prepare_texture_uploads(commands)?;
        if self.state.swapchain_recreation_requested {
            self.recreate_swapchain()?;
        }
        let mut raw_draw_states = self.preflight_frame(commands, &frame_uniform_writes)?;
        Self::begin_frame(&mut self.handle.previous_frame_end);
        let (image_index, suboptimal, acquire_future): (u32, bool, SwapchainAcquireFuture) =
            match Self::acquire_next_image_from_swapchain(&self.handle.swapchain, None) {
                Ok(acquisition) => acquisition,
                Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => {
                    self.recreate_swapchain()?;
                    raw_draw_states = self.preflight_frame(commands, &frame_uniform_writes)?;
                    Self::acquire_next_image_from_swapchain(&self.handle.swapchain, None)?
                }
                Err(error) => return Err(error),
            };
        if suboptimal {
            log::warn!(
                "{}",
                VMNLError::new(VMNLErrorKind::VulkanOutOfDate).report()
            );
            self.state.swapchain_recreation_requested = true;
        }
        let framebuffer_index: usize = usize::try_from(image_index)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        if let Some(completion) = self
            .handle
            .depth_frame_completions
            .get_mut(framebuffer_index)
            .and_then(Option::take)
        {
            // Swapchain acquisition tracks only the color image. Unlock this image's
            // previous depth use before recording/submitting another exclusive use.
            completion
                .wait(None)
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        }
        for write in frame_uniform_writes {
            write.write(framebuffer_index)?;
        }

        let command_buffer: Arc<PrimaryAutoCommandBuffer> =
            self.build_command_buffer(image_index, commands, raw_draw_states)?;
        let frame_sync = Self::frame_sync(
            &mut self.handle.previous_frame_end,
            acquire_future,
            command_buffer,
            upload_dependencies,
            image_index,
            self.handle.vmnl_instance.graphics_queue.clone(),
            self.handle.swapchain.clone(),
        );
        let device = self.handle.vmnl_instance.device.clone();
        let handle = &mut self.handle;
        sync::finalize_frame_sync(
            frame_sync,
            &mut self.state.swapchain_recreation_requested,
            |completion| {
                if let Some(completion) = completion {
                    handle.previous_frame_end = Some(completion.clone().boxed_send_sync());
                    if let Some(slot) = handle.depth_frame_completions.get_mut(framebuffer_index) {
                        *slot = Some(completion);
                    }
                } else {
                    handle.previous_frame_end =
                        Some(vulkano::sync::now(device.clone()).boxed_send_sync());
                }
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use crate::common::{
        ColorBlendConfig, ColorBlendFactor, ColorBlendOperation, ColorBlendOverrides,
    };
    use crate::raw::{self, Scissor, ScissorPolicy, Viewport, ViewportPolicy};
    use crate::texture::{
        AddressMode, AlphaMode, ColorEncoding, PixelSource, Sampler, SamplerBorderColor,
        SamplerConfig, SamplerCoordinateMode, SamplerLodLimit, Texture2D, TextureComponent,
        TextureComponentMapping, TextureDefaults, TextureFormat, TextureView2D,
        TextureViewInterpretation, UploadWaitOutcome,
    };
    use crate::{Context, DeviceFeature, VMNLError, VMNLErrorKind, VMNLResult, Window};
    use std::sync::Arc;
    use std::time::Duration;
    use vulkano::{
        buffer::{Buffer, BufferCreateInfo, BufferUsage},
        command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo},
        format::{Format, FormatFeatures},
        image::{
            view::ImageView, Image, ImageCreateFlags, ImageCreateInfo, ImageFormatInfo,
            ImageLayout, ImageTiling, ImageUsage, SampleCounts,
        },
        memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
        render_pass::{Framebuffer, FramebufferCreateInfo},
        sync::{self, GpuFuture},
    };

    const PENDING_UPLOAD_EXTENT: u32 = 2048;

    #[derive(Clone, Copy)]
    struct BlendPresetCase {
        name: &'static str,
        config: Option<ColorBlendConfig>,
        effective_config: ColorBlendConfig,
        solid_shader: &'static str,
        texture_shader: &'static str,
        expected: [u8; 4],
    }

    #[derive(Clone, Copy)]
    struct BlendReadbackCase<'a> {
        name: &'static str,
        fragment_shader: &'a str,
        config: Option<ColorBlendConfig>,
        effective_config: ColorBlendConfig,
        texture_binding: Option<(&'a Texture2D, &'a Sampler)>,
        expected: [u8; 4],
    }

    const TEXTURED_VERTEX_SHADER: &str = r"
        #version 450
        layout(location = 0) in vec2 position;
        layout(location = 1) in vec2 uv;
        layout(location = 0) out vec2 texture_uv;
        void main() {
            gl_Position = vec4(position, 0.0, 1.0);
            texture_uv = uv;
        }
    ";

    const TEXTURED_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2D color_image;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = texture(color_image, texture_uv);
        }
    ";

    const SOLID_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = vec4(1.0, 0.0, 0.0, 0.5);
        }
    ";

    const PREMULTIPLIED_SOLID_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = vec4(0.5, 0.0, 0.0, 0.5);
        }
    ";

    const PREMULTIPLIED_TEXTURE_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2D color_image;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            vec4 straight = texture(color_image, texture_uv);
            out_color = vec4(straight.rgb * straight.a, straight.a);
        }
    ";

    const TEXTURED_FRAME_UNIFORM_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2D color_image;
        layout(set = 0, binding = 1) uniform Tint { vec4 tint; };
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = texture(color_image, texture_uv) * tint;
        }
    ";

    const TEXTURED_STATIC_UNIFORM_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2D color_image;
        layout(set = 0, binding = 1) uniform Tint { vec4 tint; };
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = texture(color_image, texture_uv) * tint;
        }
    ";

    const SOLID_GREEN_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = vec4(0.0, 1.0, 0.0, 1.0);
        }
    ";

    const UNIFORM_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform Tint { vec4 tint; };
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = tint;
        }
    ";

    const SEPARATE_TEXTURE_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform texture2D color_image;
        layout(set = 1, binding = 0) uniform sampler nearest_sampler;
        layout(set = 2, binding = 0) uniform sampler linear_sampler;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        vec4 sample_with(texture2D image, sampler state, vec2 coordinates) {
            return texture(sampler2D(image, state), coordinates);
        }
        void main() {
            vec4 nearest = sample_with(color_image, nearest_sampler, texture_uv);
            vec4 linear = textureLod(
                sampler2D(color_image, linear_sampler), texture_uv, 0.0);
            out_color = vec4(nearest.r, linear.g, nearest.b, 1.0);
        }
    ";

    const SEPARATE_TEXEL_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform texture2D color_image;
        layout(set = 1, binding = 0) uniform sampler image_sampler;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = textureLod(
                sampler2D(color_image, image_sampler), texture_uv, 0.0);
        }
    ";

    const SEPARATE_NORMALIZED_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform texture2D color_image;
        layout(set = 1, binding = 0) uniform sampler image_sampler;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = texture(sampler2D(color_image, image_sampler), texture_uv);
        }
    ";

    const COMBINED_DEPTH_GATHER_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2DShadow shadow_image;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = textureGather(shadow_image, texture_uv, 0.5);
        }
    ";

    const SEPARATE_DEPTH_GATHER_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform texture2D shadow_image;
        layout(set = 1, binding = 0) uniform samplerShadow shadow_sampler;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        vec4 gather_shadow(texture2D image, samplerShadow state, vec2 uv) {
            return textureGather(sampler2DShadow(image, state), uv, 0.5);
        }
        void main() {
            out_color = gather_shadow(shadow_image, shadow_sampler, texture_uv);
        }
    ";

    const SEPARATE_IMPLICIT_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform texture2D color_image;
        layout(set = 1, binding = 0) uniform sampler image_sampler;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = texture(
                sampler2D(color_image, image_sampler), texture_uv);
        }
    ";

    const SEPARATE_NONZERO_LOD_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform texture2D color_image;
        layout(set = 1, binding = 0) uniform sampler image_sampler;
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = textureLod(
                sampler2D(color_image, image_sampler), texture_uv, 1.0);
        }
    ";

    const CONFLICTING_VERTEX_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform Transform { vec4 position; } transform;
        void main() { gl_Position = transform.position; }
    ";

    const CONFLICTING_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2D image;
        layout(location = 0) out vec4 color;
        void main() { color = texture(image, vec2(0.5)); }
    ";

    const LENGTH_ONE_ARRAY_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform sampler2D images[1];
        layout(location = 0) out vec4 color;
        void main() { color = texture(images[0], vec2(0.5)); }
    ";

    #[repr(C)]
    #[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
    struct TexturedVertex {
        #[format(R32G32_SFLOAT)]
        position: [f32; 2],
        #[format(R32G32_SFLOAT)]
        uv: [f32; 2],
    }

    #[repr(C)]
    #[derive(Clone, Copy, raw::Pod, raw::Zeroable)]
    struct TextureTint {
        tint: [f32; 4],
    }

    fn textured_quad() -> [TexturedVertex; 6] {
        [
            TexturedVertex {
                position: [-1.0, -1.0],
                uv: [0.0, 0.0],
            },
            TexturedVertex {
                position: [1.0, -1.0],
                uv: [1.0, 0.0],
            },
            TexturedVertex {
                position: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
            TexturedVertex {
                position: [-1.0, -1.0],
                uv: [0.0, 0.0],
            },
            TexturedVertex {
                position: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
            TexturedVertex {
                position: [-1.0, 1.0],
                uv: [0.0, 1.0],
            },
        ]
    }

    fn textured_quad_at_uv(uv: [f32; 2]) -> [TexturedVertex; 6] {
        let mut vertices = textured_quad();
        for vertex in &mut vertices {
            vertex.uv = uv;
        }
        vertices
    }

    fn assert_invalid_state<T>(result: VMNLResult<T>, expected: &str) {
        let Err(error) = result else {
            panic!("expected InvalidState: {expected}");
        };
        match error.kind() {
            VMNLErrorKind::InvalidState(message) if message == expected => {}
            kind => panic!("expected InvalidState({expected:?}), received {kind:?}"),
        }
    }

    fn assert_invalid_state_contains<T>(result: VMNLResult<T>, expected: &str) {
        let Err(error) = result else {
            panic!("expected InvalidState containing {expected:?}");
        };
        match error.kind() {
            VMNLErrorKind::InvalidState(message) if message.contains(expected) => {}
            kind => panic!("expected InvalidState containing {expected:?}, received {kind:?}"),
        }
    }

    fn raw_texture_pipeline(
        window: &Window,
        fragment_shader: &str,
    ) -> VMNLResult<raw::Pipeline<TexturedVertex>> {
        raw_texture_pipeline_at(window, fragment_shader, [0.0, 0.0], [0, 0])
    }

    fn raw_texture_pipeline_at(
        window: &Window,
        fragment_shader: &str,
        viewport_offset: [f32; 2],
        scissor_offset: [u32; 2],
    ) -> VMNLResult<raw::Pipeline<TexturedVertex>> {
        raw::Pipeline::<TexturedVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
            .fragment_shader(raw::ShaderSource::Src(fragment_shader.into()))
            .topology(raw::PrimitiveTopology::TriangleList)
            .viewport(ViewportPolicy::Fixed(Viewport {
                offset: viewport_offset,
                extent: [2.0, 2.0],
                depth_range: [0.0, 1.0],
            }))
            .scissor(ScissorPolicy::Fixed(Scissor {
                offset: scissor_offset,
                extent: [2, 2],
            }))
            .build(window)
    }

    fn raw_blend_pipeline(
        window: &Window,
        fragment_shader: &str,
        config: Option<crate::common::ColorBlendConfig>,
    ) -> VMNLResult<raw::Pipeline<TexturedVertex>> {
        let spec = raw::Pipeline::<TexturedVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
            .fragment_shader(raw::ShaderSource::Src(fragment_shader.into()))
            .topology(raw::PrimitiveTopology::TriangleList)
            .viewport(ViewportPolicy::Fixed(Viewport {
                offset: [0.0, 0.0],
                extent: [2.0, 2.0],
                depth_range: [0.0, 1.0],
            }))
            .scissor(ScissorPolicy::Fixed(Scissor {
                offset: [0, 0],
                extent: [2, 2],
            }));
        match config {
            Some(config) => spec.color_blend(config).build(window),
            None => spec.blend_mode(raw::BlendMode::Alpha).build(window),
        }
    }

    fn assert_raw_blend_output(
        window: &mut Window,
        context: &Context,
        target: &Arc<Image>,
        row_stride: usize,
        geometry: &raw::Geometry<TexturedVertex>,
        case: BlendReadbackCase<'_>,
    ) -> VMNLResult<()> {
        let pipeline = raw_blend_pipeline(window, case.fragment_shader, case.config)?;
        assert_eq!(
            pipeline.color_blend_config_value(),
            case.effective_config,
            "effective {} config",
            case.name
        );
        let builder = raw::Resources::builder(&pipeline);
        let resources = match case.texture_binding {
            Some((texture, sampler)) => builder.sampled_texture(0, 0, texture, sampler),
            None => builder,
        }
        .build(context)?;
        window
            .render()
            .draw_raw_2d_with(&pipeline, &resources, [geometry])
            .submit()?;
        let readback = readback_first_2x2(window, target.clone());
        assert_first_2x2_color(&readback, row_stride, case.expected);
        Ok(())
    }

    fn raw_blend_preset_cases() -> [BlendPresetCase; 4] {
        let legacy = ColorBlendConfig::straight_alpha().with_alpha_blend(
            ColorBlendFactor::SourceAlpha,
            ColorBlendFactor::OneMinusSourceAlpha,
            ColorBlendOperation::Add,
        );
        [
            BlendPresetCase {
                name: "opaque",
                config: Some(ColorBlendConfig::opaque()),
                effective_config: ColorBlendConfig::opaque(),
                solid_shader: SOLID_FRAGMENT_SHADER,
                texture_shader: TEXTURED_FRAGMENT_SHADER,
                expected: [255, 0, 0, 128],
            },
            BlendPresetCase {
                name: "straight-alpha",
                config: Some(ColorBlendConfig::straight_alpha()),
                effective_config: ColorBlendConfig::straight_alpha(),
                solid_shader: SOLID_FRAGMENT_SHADER,
                texture_shader: TEXTURED_FRAGMENT_SHADER,
                expected: [128, 0, 0, 128],
            },
            BlendPresetCase {
                name: "premultiplied-alpha",
                config: Some(ColorBlendConfig::premultiplied_alpha()),
                effective_config: ColorBlendConfig::premultiplied_alpha(),
                solid_shader: PREMULTIPLIED_SOLID_FRAGMENT_SHADER,
                texture_shader: PREMULTIPLIED_TEXTURE_FRAGMENT_SHADER,
                expected: [128, 0, 0, 128],
            },
            BlendPresetCase {
                name: "legacy-alpha",
                config: None,
                effective_config: legacy,
                solid_shader: SOLID_FRAGMENT_SHADER,
                texture_shader: TEXTURED_FRAGMENT_SHADER,
                expected: [128, 0, 0, 64],
            },
        ]
    }

    fn raw_test_texture(context: &Context) -> VMNLResult<Texture2D> {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, // top row: red, green
            0, 0, 255, 255, 255, 255, 255, 255, // bottom row: blue, white
        ];
        Texture2D::from_pixels(
            context,
            PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &pixels)?,
        )
        .build()
    }

    fn assert_foreign_texture_context_is_rejected(
        context: &Context,
        pipeline: &raw::Pipeline<TexturedVertex>,
    ) -> VMNLResult<()> {
        let foreign_context = Context::new()?;
        let foreign_texture = raw_test_texture(&foreign_context)?;
        let foreign_sampler = Sampler::builder(&foreign_context)
            .config(SamplerConfig::nearest())
            .build()?;
        assert_invalid_state_contains(
            raw::Resources::builder(pipeline)
                .sampled_texture(0, 0, &foreign_texture, &foreign_sampler)
                .build(context),
            "must belong to this context",
        );
        Ok(())
    }

    fn assert_raw_pipeline_layout_rejections(window: &Window) {
        assert_invalid_state_contains(
            raw::Pipeline::<TexturedVertex>::builder()
                .vertex_shader(raw::ShaderSource::Src(CONFLICTING_VERTEX_SHADER.into()))
                .fragment_shader(raw::ShaderSource::Src(CONFLICTING_FRAGMENT_SHADER.into()))
                .build(window),
            "conflict at set 0 binding 0",
        );
        assert_invalid_state_contains(
            raw::Pipeline::<TexturedVertex>::builder()
                .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
                .fragment_shader(raw::ShaderSource::Src(
                    LENGTH_ONE_ARRAY_FRAGMENT_SHADER.into(),
                ))
                .build(window),
            "set 0 binding 0 descriptor arrays are not supported yet",
        );
    }

    fn install_rgba8_readback_target(window: &mut Window) -> Option<Arc<Image>> {
        let device = window.device();
        let format = Format::R8G8B8A8_UNORM;
        let usage = ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC;
        let properties = device
            .physical_device()
            .image_format_properties(ImageFormatInfo {
                format,
                usage,
                ..Default::default()
            })
            .ok()
            .flatten()?;
        let extent = window.inner.handle.swapchain.image_extent();
        if extent[0] > properties.max_extent[0]
            || extent[1] > properties.max_extent[1]
            || !properties.sample_counts.contains(SampleCounts::SAMPLE_1)
        {
            return None;
        }
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    format: format,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                    final_layout: ImageLayout::TransferSrcOptimal,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {},
            },
        )
        .expect("RGBA8 readback render pass should be supported");
        let image = Image::new(
            window.memory_allocator(),
            ImageCreateInfo {
                format,
                extent: [extent[0], extent[1], 1],
                usage,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .expect("RGBA8 readback image should allocate");
        let image_view = ImageView::new_default(image.clone())
            .expect("RGBA8 readback image view should allocate");
        let framebuffer = Framebuffer::new(
            render_pass.clone(),
            FramebufferCreateInfo {
                attachments: vec![image_view],
                extent,
                layers: 1,
                ..Default::default()
            },
        )
        .expect("RGBA8 readback framebuffer should allocate");

        window.inner.handle.render_pass = render_pass.clone();
        let swapchain_image_count = usize::try_from(window.inner.handle.swapchain.image_count())
            .expect("swapchain image count should fit usize");
        window.inner.handle.framebuffers = vec![framebuffer; swapchain_image_count];
        window.inner.config.render_target_info =
            crate::RenderTargetInfo::from_render_pass(&render_pass)
                .expect("readback render target properties should resolve");
        Some(image)
    }

    fn supports_mutable_rgba8_srgb_view(context: &Context) -> bool {
        let device = &context.inner.device;
        let unorm = Format::R8G8B8A8_UNORM;
        let srgb = Format::R8G8B8A8_SRGB;
        let view_formats = if device.api_version() >= vulkano::Version::V1_2
            || device.enabled_extensions().khr_image_format_list
        {
            vec![unorm, srgb]
        } else {
            Vec::new()
        };
        let Ok(srgb_properties) = context.inner.physical_device.format_properties(srgb) else {
            return false;
        };
        if !srgb_properties
            .optimal_tiling_features
            .contains(FormatFeatures::SAMPLED_IMAGE)
        {
            return false;
        }

        context
            .inner
            .physical_device
            .image_format_properties(ImageFormatInfo {
                flags: ImageCreateFlags::MUTABLE_FORMAT,
                format: unorm,
                view_formats,
                tiling: ImageTiling::Optimal,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
                ..Default::default()
            })
            .is_ok_and(|properties| properties.is_some())
    }

    fn install_rgba8_blend_target(window: &mut Window) -> VMNLResult<Option<Arc<Image>>> {
        let Some(target) = install_rgba8_readback_target(window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(None);
        };
        let format_features = window
            .device()
            .physical_device()
            .format_properties(Format::R8G8B8A8_UNORM)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?
            .optimal_tiling_features;
        if !format_features.contains(vulkano::format::FormatFeatures::COLOR_ATTACHMENT_BLEND) {
            log::info!("R8G8B8A8_UNORM blending is unsupported; skipped");
            return Ok(None);
        }
        Ok(Some(target))
    }

    fn readback_first_2x2(window: &mut Window, image: Arc<Image>) -> Vec<u8> {
        let [width, height] = window.inner.handle.swapchain.image_extent();
        let byte_count = usize::try_from(width)
            .expect("width should fit usize")
            .checked_mul(usize::try_from(height).expect("height should fit usize"))
            .and_then(|pixels| pixels.checked_mul(4))
            .expect("readback byte count should fit usize");
        let buffer = Buffer::new_slice::<u8>(
            window.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            u64::try_from(byte_count).expect("readback byte count should fit u64"),
        )
        .expect("readback buffer should allocate");
        let mut command_buffer = AutoCommandBufferBuilder::primary(
            window
                .inner
                .handle
                .vmnl_instance
                .command_buffer_allocator
                .clone(),
            window
                .inner
                .handle
                .vmnl_instance
                .graphics_queue_family_index,
            CommandBufferUsage::OneTimeSubmit,
        )
        .expect("readback command buffer should allocate");
        let mut copy = CopyImageToBufferInfo::image_buffer(image, buffer.clone());
        copy.src_image_layout = ImageLayout::TransferSrcOptimal;
        command_buffer
            .copy_image_to_buffer(copy)
            .expect("readback copy should record");
        let command_buffer = command_buffer
            .build()
            .expect("readback command buffer should build");

        let device = window.device();
        let queue = window.inner.handle.vmnl_instance.graphics_queue.clone();
        window
            .inner
            .handle
            .previous_frame_end
            .take()
            .expect("submitted frame should retain its completion")
            .then_execute(queue, command_buffer)
            .expect("readback should submit")
            .then_signal_fence_and_flush()
            .expect("readback fence should signal")
            .wait(None)
            .expect("readback should complete");
        window.inner.handle.previous_frame_end = Some(sync::now(device).boxed_send_sync());

        let pixels = buffer.read().expect("readback buffer should map").to_vec();
        pixels
    }

    fn assert_first_2x2(readback: &[u8], row_stride: usize) {
        let observed = [
            readback[0..4].to_vec(),
            readback[4..8].to_vec(),
            readback[row_stride..row_stride + 4].to_vec(),
            readback[row_stride + 4..row_stride + 8].to_vec(),
        ];
        assert_eq!(
            observed,
            [
                vec![255, 0, 0, 255],
                vec![0, 255, 0, 255],
                vec![0, 0, 255, 255],
                vec![255, 255, 255, 255],
            ],
        );
    }

    fn assert_first_2x2_color(readback: &[u8], row_stride: usize, expected: [u8; 4]) {
        let observed = [
            &readback[0..4],
            &readback[4..8],
            &readback[row_stride..row_stride + 4],
            &readback[row_stride + 4..row_stride + 8],
        ];
        for pixel in observed {
            assert_eq!(pixel, expected);
        }
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_sampled_texture_joins_upload_and_reads_rgba8_texels() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().build(&context)?;
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };

        let pipeline = raw::Pipeline::<TexturedVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
            .fragment_shader(raw::ShaderSource::Src(TEXTURED_FRAGMENT_SHADER.into()))
            .topology(raw::PrimitiveTopology::TriangleList)
            .viewport(ViewportPolicy::Fixed(Viewport {
                offset: [0.0, 0.0],
                extent: [2.0, 2.0],
                depth_range: [0.0, 1.0],
            }))
            .scissor(ScissorPolicy::Fixed(Scissor {
                offset: [0, 0],
                extent: [2, 2],
            }))
            .build(&window)?;
        let offcenter_pipeline = raw::Pipeline::<TexturedVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
            .fragment_shader(raw::ShaderSource::Src(TEXTURED_FRAGMENT_SHADER.into()))
            .topology(raw::PrimitiveTopology::TriangleList)
            .viewport(ViewportPolicy::Fixed(Viewport {
                offset: [2.0, 0.0],
                extent: [1.0, 1.0],
                depth_range: [0.0, 1.0],
            }))
            .scissor(ScissorPolicy::Fixed(Scissor {
                offset: [2, 0],
                extent: [1, 1],
            }))
            .build(&window)?;
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, // top row: red, green
            0, 0, 255, 255, 255, 255, 255, 255, // bottom row: blue, white
        ];
        let texture = Texture2D::from_pixels(
            &context,
            PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &pixels)?,
        )
        .build()?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;

        assert_invalid_state(
            raw::Resources::builder(&pipeline).build(&context),
            "raw resources missing set 0 binding 0",
        );
        assert_invalid_state(
            raw::Resources::builder(&pipeline)
                .sampled_texture(0, 0, &texture, &sampler)
                .sampled_texture(0, 0, &texture, &sampler)
                .build(&context),
            "raw resources duplicate binding set 0 binding 0",
        );
        let wrong_type_pipeline = raw::Pipeline::<TexturedVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
            .fragment_shader(raw::ShaderSource::Src(UNIFORM_FRAGMENT_SHADER.into()))
            .build(&window)?;
        assert_invalid_state(
            raw::Resources::builder(&wrong_type_pipeline)
                .sampled_texture(0, 0, &texture, &sampler)
                .build(&context),
            "raw resources set 0 binding 0 resource does not match the pipeline descriptor type",
        );

        let resources = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)?;

        assert_foreign_texture_context_is_rejected(&context, &pipeline)?;

        let offcenter_resources = raw::Resources::builder(&offcenter_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;
        let offcenter_geometry =
            raw::Geometry::builder(textured_quad_at_uv([0.375, 0.25])).build(&context)?;
        let row_stride = usize::try_from(window.inner.handle.swapchain.image_extent()[0])
            .expect("width should fit usize")
            .checked_mul(4)
            .expect("row stride should fit usize");
        for _ in 0..2 {
            window
                .render()
                .draw_raw_2d_with(&pipeline, &resources, [&geometry, &geometry])
                .draw_raw_2d_with(
                    &offcenter_pipeline,
                    &offcenter_resources,
                    [&offcenter_geometry],
                )
                .submit()?;
            let readback = readback_first_2x2(&mut window, target.clone());
            assert_first_2x2(&readback, row_stride);
            // u=0.375 is one quarter between the red and green texel centers.
            // Nearest therefore returns red; Linear would blend the two texels.
            assert_eq!(&readback[8..12], &[255, 0, 0, 255]);
        }

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_failed_upload_rejections_leave_swapchain_available() -> VMNLResult<()> {
        use super::acquire::ACQUIRE_ATTEMPTS;

        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().build(&context)?;
        let pipeline = raw_texture_pipeline(&window, TEXTURED_FRAME_UNIFORM_FRAGMENT_SHADER)?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let tint = TextureTint { tint: [1.0; 4] };
        let uniform = raw::Uniform::builder(tint).build(&context)?;
        let frame_uniform = raw::FrameUniform::builder(tint).build(&window)?;
        let failed = raw_test_texture(&context)?;
        let valid = raw_test_texture(&context)?;
        let failed_static = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &failed, &sampler)
            .uniform(0, 1, &uniform)
            .build(&context)?;
        let failed_dynamic = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &failed, &sampler)
            .frame_uniform(0, 1, &frame_uniform)
            .build(&context)?;
        let valid_resources = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &valid, &sampler)
            .uniform(0, 1, &uniform)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;

        // Inject the status after Resources construction, without losing the Vulkan device.
        let message = "simulated initial upload failure";
        failed.simulate_initial_upload_failure(message);
        let attempts_before = ACQUIRE_ATTEMPTS.get();
        for resources in [&failed_static, &failed_dynamic] {
            let error = window
                .render()
                .draw_raw_2d_with(&pipeline, resources, [&geometry])
                .submit()
                .expect_err("known upload failure must reject submission");
            assert!(matches!(
                error.kind(),
                VMNLErrorKind::TextureUploadFailed(reason) if reason == message
            ));
            assert_eq!(ACQUIRE_ATTEMPTS.get(), attempts_before);
        }
        let error = failed
            .initial_upload()
            .is_complete()
            .expect_err("the ticket must observe the same failure");
        assert!(matches!(
            error.kind(),
            VMNLErrorKind::TextureUploadFailed(reason) if reason == message
        ));
        window
            .render()
            .draw_raw_2d_with(&pipeline, &valid_resources, [&geometry])
            .submit()?;
        assert!(ACQUIRE_ATTEMPTS.get() > attempts_before);

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_pipeline_resolves_blend_defaults_at_build() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let defaults = ColorBlendOverrides::from_config(ColorBlendConfig::straight_alpha());
        let context = Context::builder().raw_color_blend(defaults).build()?;
        let window = Window::builder().build(&context)?;
        let spec = raw::Pipeline::<TexturedVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(TEXTURED_VERTEX_SHADER.into()))
            .fragment_shader(raw::ShaderSource::Src(SOLID_GREEN_FRAGMENT_SHADER.into()));
        let inherited = spec.clone().build(&window)?;
        assert_eq!(
            inherited.color_blend_config_value(),
            context.raw_color_blend_config()
        );
        let partial = spec
            .clone()
            .color_blend_overrides(ColorBlendOverrides::default().with_blend_enabled(false))
            .build(&window)?;
        assert_eq!(
            partial.color_blend_config_value(),
            ColorBlendConfig::straight_alpha().with_blend_enabled(false)
        );
        let explicit = spec.blend_mode(raw::BlendMode::Opaque).build(&window)?;
        assert_eq!(
            explicit.color_blend_config_value(),
            ColorBlendConfig::opaque()
        );
        assert_eq!(
            inherited.color_blend_config_value(),
            ColorBlendConfig::straight_alpha()
        );
        assert_eq!(context.clone().raw_color_blend_defaults(), defaults);
        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_frame_preflight_rejects_resources_from_another_pipeline() -> VMNLResult<()> {
        use super::acquire::ACQUIRE_ATTEMPTS;

        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().build(&context)?;
        let pipeline = raw_texture_pipeline(&window, TEXTURED_FRAGMENT_SHADER)?;
        let other_pipeline = raw_texture_pipeline(&window, TEXTURED_FRAGMENT_SHADER)?;
        let texture = raw_test_texture(&context)?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let wrong_resources = raw::Resources::builder(&other_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)?;
        let resources = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;
        let attempts_before = ACQUIRE_ATTEMPTS.get();

        for _ in 0..2 {
            assert_invalid_state_contains(
                window
                    .render()
                    .draw_raw_2d_with(&pipeline, &wrong_resources, [&geometry])
                    .submit(),
                "raw resources must be built for this raw pipeline",
            );
            assert_eq!(ACQUIRE_ATTEMPTS.get(), attempts_before);
        }
        window
            .render()
            .draw_raw_2d_with(&pipeline, &resources, [&geometry])
            .submit()?;
        assert!(ACQUIRE_ATTEMPTS.get() > attempts_before);

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_frame_preflight_rejects_stale_frame_uniforms() -> VMNLResult<()> {
        use super::acquire::ACQUIRE_ATTEMPTS;

        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().build(&context)?;
        let tint = TextureTint { tint: [1.0; 4] };
        // Simulate a stale FrameUniform without depending on a driver changing its image count.
        let framebuffers = &mut window.inner.handle.framebuffers;
        framebuffers.push(framebuffers[0].clone());
        let stale = raw::FrameUniform::builder(tint).build(&window);
        window.inner.handle.framebuffers.pop();
        let mut stale = stale?;
        let mut valid = raw::FrameUniform::builder(tint).build(&window)?;
        let pipeline = raw_texture_pipeline(&window, TEXTURED_FRAME_UNIFORM_FRAGMENT_SHADER)?;
        let texture = raw_test_texture(&context)?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let resources = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .frame_uniform(0, 1, &valid)
            .build(&context)?;
        let stale_resources = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .frame_uniform(0, 1, &stale)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;
        let attempts_before = ACQUIRE_ATTEMPTS.get();
        let mismatch =
            "raw frame uniform image count is incompatible with current swapchain image count";

        assert_invalid_state_contains(
            window
                .render()
                .draw_raw_2d_with(&pipeline, &stale_resources, [&geometry])
                .submit(),
            mismatch,
        );
        assert_eq!(ACQUIRE_ATTEMPTS.get(), attempts_before);
        assert_invalid_state_contains(
            window
                .render()
                .write_frame_uniform(&mut stale, tint)
                .submit(),
            mismatch,
        );
        assert_eq!(ACQUIRE_ATTEMPTS.get(), attempts_before);
        window
            .render()
            .write_frame_uniform(&mut valid, tint)
            .draw_raw_2d_with(&pipeline, &resources, [&geometry])
            .submit()?;
        assert!(ACQUIRE_ATTEMPTS.get() > attempts_before);

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_texture_with_static_uniform_survives_source_drop_before_readback() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().size(640, 480).build(&context)?;
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };
        let [target_width, target_height] = window.inner.handle.swapchain.image_extent();
        if target_width < 4 || target_height < 2 {
            log::info!("readback target is smaller than the mixed-resource test layout; skipped");
            return Ok(());
        }

        let texture = raw_test_texture(&context)?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let textured_pipeline = raw_texture_pipeline_at(
            &window,
            TEXTURED_STATIC_UNIFORM_FRAGMENT_SHADER,
            [0.0, 0.0],
            [0, 0],
        )?;
        let color_only_pipeline =
            raw_texture_pipeline_at(&window, SOLID_GREEN_FRAGMENT_SHADER, [2.0, 0.0], [2, 0])?;
        let uniform = raw::Uniform::builder(TextureTint {
            tint: [0.5, 0.0, 1.0, 1.0],
        })
        .build(&context)?;
        let resources = raw::Resources::builder(&textured_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .uniform(0, 1, &uniform)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad_at_uv([0.25, 0.25])).build(&context)?;

        window
            .render()
            .draw_raw_2d_with(&textured_pipeline, &resources, [&geometry])
            .draw_raw_2d(&color_only_pipeline, [&geometry])
            .submit()?;

        drop(geometry);
        drop(uniform);
        drop(sampler);
        drop(texture);
        drop(resources);
        drop(textured_pipeline);
        drop(color_only_pipeline);

        let readback = readback_first_2x2(&mut window, target);
        let row_stride = usize::try_from(target_width)
            .expect("target width fits usize")
            .checked_mul(4)
            .expect("row stride fits usize");
        for y in 0..2 {
            let row_start = usize::try_from(y)
                .expect("small row index fits usize")
                .checked_mul(row_stride)
                .expect("row offset fits usize");
            for x in 0..4 {
                let pixel_start = row_start + usize::try_from(x).expect("small x fits usize") * 4;
                let expected = if x < 2 {
                    [128, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                };
                assert_eq!(&readback[pixel_start..pixel_start + 4], &expected);
            }
        }

        Ok(())
    }

    fn explicit_swizzled_view(
        context: &Context,
        texture: &Texture2D,
    ) -> VMNLResult<Option<TextureView2D>> {
        let swizzle = TextureComponentMapping::new(
            TextureComponent::Blue,
            TextureComponent::Green,
            TextureComponent::Red,
            TextureComponent::Alpha,
        );
        if context
            .inner
            .device
            .enabled_extensions()
            .khr_portability_subset
            && !context.is_device_feature_enabled(DeviceFeature::ImageViewFormatSwizzle)
        {
            assert_invalid_state_contains(
                TextureView2D::builder(texture)
                    .component_mapping(swizzle)
                    .build(),
                "requires enabled ImageViewFormatSwizzle",
            );
            return Ok(None);
        }

        let view = TextureView2D::builder(texture)
            .component_mapping(swizzle)
            .build()?;
        assert!(view.shares_storage_with(texture));
        assert_eq!(view.requested_component_mapping(), Some(swizzle));
        assert_eq!(view.effective_component_mapping(), swizzle);
        assert_eq!(view.sampled_interpretation(), None);

        let opaque_border_sampler = Sampler::builder(context)
            .config(
                SamplerConfig::nearest()
                    .with_address_u(AddressMode::ClampToBorder)
                    .with_border_color(SamplerBorderColor::FloatOpaqueBlack),
            )
            .build()?;
        assert_invalid_state_contains(
            view.validate_sampler(&opaque_border_sampler),
            "opaque-black border sampling with a non-identity texture view mapping is unsupported",
        );
        assert_invalid_state_contains(
            TextureView2D::builder(texture)
                .format(TextureFormat::Rgba8Srgb)
                .build(),
            "was not enabled when the image was created",
        );

        Ok(Some(view))
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_explicit_texture_views_apply_component_swizzles() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().size(640, 480).build(&context)?;
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };
        let [target_width, target_height] = window.inner.handle.swapchain.image_extent();
        if target_width < 4 || target_height < 4 {
            log::info!("readback target is smaller than the view test layout; skipped");
            return Ok(());
        }

        let texture = raw_test_texture(&context)?;
        let default_view = texture.view();
        assert_eq!(default_view.effective_format(), TextureFormat::Rgba8Unorm);
        assert_eq!(
            default_view.sampled_interpretation(),
            Some(TextureViewInterpretation::new(
                texture.encoding(),
                texture.alpha_mode(),
            ))
        );

        let Some(swizzled_view) = explicit_swizzled_view(&context, &texture)? else {
            return Ok(());
        };

        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let default_pipeline =
            raw_texture_pipeline_at(&window, TEXTURED_FRAGMENT_SHADER, [0.0, 0.0], [0, 0])?;
        let combined_view_pipeline =
            raw_texture_pipeline_at(&window, TEXTURED_FRAGMENT_SHADER, [2.0, 0.0], [2, 0])?;
        let separate_view_pipeline =
            raw_texture_pipeline_at(&window, SEPARATE_TEXEL_FRAGMENT_SHADER, [0.0, 2.0], [0, 2])?;
        assert_invalid_state_contains(
            raw::Resources::builder(&combined_view_pipeline)
                .sampled_texture_view(0, 0, &swizzled_view, &sampler)
                .sampled_texture_view(0, 0, &swizzled_view, &sampler)
                .build(&context),
            "duplicate binding set 0 binding 0",
        );
        let default_resources = raw::Resources::builder(&default_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)?;
        let combined_view_resources = raw::Resources::builder(&combined_view_pipeline)
            .sampled_texture_view(0, 0, &swizzled_view, &sampler)
            .build(&context)?;
        let separate_view_resources = raw::Resources::builder(&separate_view_pipeline)
            .texture_view(0, 0, &swizzled_view)
            .sampler(1, 0, &sampler)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;

        window
            .render()
            .draw_raw_2d_with(&default_pipeline, &default_resources, [&geometry])
            .draw_raw_2d_with(
                &combined_view_pipeline,
                &combined_view_resources,
                [&geometry],
            )
            .draw_raw_2d_with(
                &separate_view_pipeline,
                &separate_view_resources,
                [&geometry],
            )
            .submit()?;

        let readback = readback_first_2x2(&mut window, target);
        let row_stride = usize::try_from(target_width)
            .expect("target width fits usize")
            .checked_mul(4)
            .expect("row stride fits usize");
        let bottom_left_pixel = row_stride * 2;
        assert_eq!(&readback[0..4], &[255, 0, 0, 255]);
        assert_eq!(&readback[8..12], &[0, 0, 255, 255]);
        assert_eq!(
            &readback[bottom_left_pixel..bottom_left_pixel + 4],
            &[0, 0, 255, 255]
        );

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn texture_and_explicit_view_resolve_context_defaults_at_build() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let interpretation = TextureViewInterpretation::new(ColorEncoding::Srgb, AlphaMode::Opaque);
        let mapping = TextureComponentMapping::new(
            TextureComponent::Blue,
            TextureComponent::Green,
            TextureComponent::Red,
            TextureComponent::Alpha,
        );
        let defaults = TextureDefaults::default()
            .with_mutable_format_views(Some(true))
            .with_view_format(Some(TextureFormat::Rgba8Srgb))
            .with_view_component_mapping(Some(mapping))
            .with_view_interpretation(Some(interpretation));
        let context = Context::builder().texture_defaults(defaults).build()?;
        assert_eq!(context.texture_defaults(), defaults);

        let pixels = [128, 0, 0, 255];
        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)?;
        let builder = Texture2D::from_pixels(&context, source).with_mutable_format_views(false);
        assert_eq!(builder.mutable_format_views_override(), Some(false));
        let fixed = builder.build()?;
        assert!(!fixed.mutable_format_views_enabled());
        let default_view = fixed.view();
        assert_eq!(default_view.effective_format(), TextureFormat::Rgba8Unorm);
        assert_eq!(
            default_view.effective_component_mapping(),
            TextureComponentMapping::identity()
        );
        assert_invalid_state_contains(
            TextureView2D::builder(&fixed).build(),
            "was not enabled when the image was created",
        );
        let local = TextureView2D::builder(&fixed)
            .format(TextureFormat::Rgba8Unorm)
            .component_mapping(TextureComponentMapping::identity())
            .build()?;
        assert_eq!(local.requested_interpretation(), None);
        assert_eq!(local.sampled_interpretation(), Some(interpretation));

        let builder = Texture2D::from_pixels(
            &context,
            PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)?,
        );
        assert_eq!(builder.mutable_format_views_override(), None);
        if !supports_mutable_rgba8_srgb_view(&context) {
            assert!(builder.prepare().is_err());
            return Ok(());
        }
        let inherited = builder.build()?;
        assert!(inherited.mutable_format_views_enabled());
        let view = TextureView2D::builder(&inherited)
            .component_mapping(TextureComponentMapping::identity())
            .use_default_interpretation()
            .build()?;
        assert_eq!(view.requested_format(), None);
        assert_eq!(view.effective_format(), TextureFormat::Rgba8Srgb);
        assert!(view.uses_default_interpretation());
        assert_eq!(view.sampled_interpretation(), None);
        if context
            .inner
            .device
            .enabled_extensions()
            .khr_portability_subset
            && !context.is_device_feature_enabled(DeviceFeature::ImageViewFormatSwizzle)
        {
            assert_invalid_state_contains(
                TextureView2D::builder(&inherited).build(),
                "requires enabled ImageViewFormatSwizzle",
            );
        } else {
            let view = TextureView2D::builder(&inherited).build()?;
            assert_eq!(view.requested_component_mapping(), None);
            assert_eq!(view.effective_component_mapping(), mapping);
            assert_eq!(view.sampled_interpretation(), Some(interpretation));
        }
        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn mutable_srgb_view_reinterprets_the_same_rgba8_storage() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        if !supports_mutable_rgba8_srgb_view(&context) {
            log::info!("mutable RGBA8 UNORM/sRGB sampled views are unsupported; skipped");
            return Ok(());
        }

        let mut window = Window::builder().size(640, 480).build(&context)?;
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };
        let [target_width, target_height] = window.inner.handle.swapchain.image_extent();
        if target_width < 4 || target_height < 2 {
            log::info!("readback target is smaller than the format-view layout; skipped");
            return Ok(());
        }

        let pixels = [128, 0, 0, 255];
        let builder = Texture2D::from_pixels(
            &context,
            PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)?,
        );
        assert!(!builder.mutable_format_views_requested());
        let builder = builder.with_mutable_format_views(true);
        assert!(builder.mutable_format_views_requested());
        let prepared = builder.prepare()?;
        assert!(prepared.mutable_format_views_enabled());
        let texture = prepared.submit()?;
        assert!(texture.mutable_format_views_enabled());
        assert!(texture.supports_view_format(TextureFormat::Rgba8Unorm));
        assert!(texture.supports_view_format(TextureFormat::Rgba8Srgb));

        let interpretation = TextureViewInterpretation::new(ColorEncoding::Srgb, AlphaMode::Opaque);
        let view = TextureView2D::builder(&texture)
            .format(TextureFormat::Rgba8Srgb)
            .interpretation(interpretation)
            .build()?;
        assert!(view.shares_storage_with(&texture));
        assert_eq!(view.requested_format(), Some(TextureFormat::Rgba8Srgb));
        assert_eq!(view.effective_format(), TextureFormat::Rgba8Srgb);
        assert_eq!(view.requested_interpretation(), Some(interpretation));
        assert_eq!(view.sampled_interpretation(), Some(interpretation));

        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let unorm_pipeline =
            raw_texture_pipeline_at(&window, TEXTURED_FRAGMENT_SHADER, [0.0, 0.0], [0, 0])?;
        let srgb_pipeline =
            raw_texture_pipeline_at(&window, TEXTURED_FRAGMENT_SHADER, [2.0, 0.0], [2, 0])?;
        let unorm_resources = raw::Resources::builder(&unorm_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)?;
        let srgb_resources = raw::Resources::builder(&srgb_pipeline)
            .sampled_texture_view(0, 0, &view, &sampler)
            .build(&context)?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;

        window
            .render()
            .draw_raw_2d_with(&unorm_pipeline, &unorm_resources, [&geometry])
            .draw_raw_2d_with(&srgb_pipeline, &srgb_resources, [&geometry])
            .submit()?;

        let readback = readback_first_2x2(&mut window, target);
        assert_eq!(&readback[0..4], &[128, 0, 0, 255]);
        let srgb_red = readback[8];
        assert!(
            (54..=56).contains(&srgb_red),
            "unexpected sRGB sample: {srgb_red}"
        );
        assert_eq!(&readback[9..12], &[0, 0, 255]);

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_color_blend_presets_match_rgba8_with_and_without_textures() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder()
            .set_clear_color([0, 0, 0, 0])
            .build(&context)?;
        let Some(target) = install_rgba8_blend_target(&mut window)? else {
            return Ok(());
        };

        let pixels = [
            255, 0, 0, 128, 255, 0, 0, 128, // top row
            255, 0, 0, 128, 255, 0, 0, 128, // bottom row
        ];
        let texture = Texture2D::from_pixels(
            &context,
            PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &pixels)?,
        )
        .build()?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let geometry = raw::Geometry::builder(textured_quad()).build(&context)?;
        let row_stride = usize::try_from(window.inner.handle.swapchain.image_extent()[0])
            .expect("width should fit usize")
            .checked_mul(4)
            .expect("row stride should fit usize");

        for preset in raw_blend_preset_cases() {
            let readback_cases = [
                BlendReadbackCase {
                    name: preset.name,
                    fragment_shader: preset.solid_shader,
                    config: preset.config,
                    effective_config: preset.effective_config,
                    texture_binding: None,
                    expected: preset.expected,
                },
                BlendReadbackCase {
                    name: preset.name,
                    fragment_shader: preset.texture_shader,
                    config: preset.config,
                    effective_config: preset.effective_config,
                    texture_binding: Some((&texture, &sampler)),
                    expected: preset.expected,
                },
            ];
            for case in readback_cases {
                assert_raw_blend_output(
                    &mut window,
                    &context,
                    &target,
                    row_stride,
                    &geometry,
                    case,
                )?;
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_separate_sampled_resources_pair_distinct_samplers() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().build(&context)?;
        assert_raw_pipeline_layout_rejections(&window);
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };

        let texture = raw_test_texture(&context)?;
        let nearest_sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let linear_sampler = Sampler::builder(&context)
            .config(SamplerConfig::linear())
            .build()?;

        let separate_pipeline = raw_texture_pipeline(&window, SEPARATE_TEXTURE_FRAGMENT_SHADER)?;
        assert_invalid_state_contains(
            raw::Resources::builder(&separate_pipeline)
                .texture(0, 0, &texture)
                .sampler(1, 0, &nearest_sampler)
                .build(&context),
            "missing set 2 binding 0",
        );
        assert_invalid_state_contains(
            raw::Resources::builder(&separate_pipeline)
                .texture(0, 0, &texture)
                .sampler(1, 0, &nearest_sampler)
                .sampler(1, 0, &linear_sampler)
                .sampler(2, 0, &linear_sampler)
                .build(&context),
            "duplicate binding set 1 binding 0",
        );
        assert_invalid_state_contains(
            raw::Resources::builder(&separate_pipeline)
                .sampler(0, 0, &nearest_sampler)
                .sampler(1, 0, &nearest_sampler)
                .sampler(2, 0, &linear_sampler)
                .build(&context),
            "set 0 binding 0 resource does not match",
        );
        let separate_resources = raw::Resources::builder(&separate_pipeline)
            .texture(0, 0, &texture)
            .sampler(1, 0, &nearest_sampler)
            .sampler(2, 0, &linear_sampler)
            .build(&context)?;
        let separate_geometry =
            raw::Geometry::builder(textured_quad_at_uv([0.375, 0.25])).build(&context)?;

        window
            .render()
            .draw_raw_2d_with(
                &separate_pipeline,
                &separate_resources,
                [&separate_geometry],
            )
            .submit()?;
        let readback = readback_first_2x2(&mut window, target);
        assert_eq!(&readback[0..4], &[255, 64, 0, 255]);

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_combined_and_separate_bindings_distinguish_linear_filtering() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().size(640, 480).build(&context)?;
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };
        let [target_width, target_height] = window.inner.handle.swapchain.image_extent();
        if target_width < 8 || target_height < 2 {
            log::info!("readback target is smaller than the filter test layout; skipped");
            return Ok(());
        }

        let texture = raw_test_texture(&context)?;
        let nearest_sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let linear_sampler = Sampler::builder(&context)
            .config(SamplerConfig::linear())
            .build()?;
        if let Err(error) = texture.view().validate_sampler(&linear_sampler) {
            log::info!("linear filtering is unsupported for the test image: {error}; skipped");
            return Ok(());
        }

        let combined_nearest_pipeline =
            raw_texture_pipeline_at(&window, TEXTURED_FRAGMENT_SHADER, [0.0, 0.0], [0, 0])?;
        let combined_linear_pipeline =
            raw_texture_pipeline_at(&window, TEXTURED_FRAGMENT_SHADER, [2.0, 0.0], [2, 0])?;
        let separate_nearest_pipeline = raw_texture_pipeline_at(
            &window,
            SEPARATE_NORMALIZED_FRAGMENT_SHADER,
            [4.0, 0.0],
            [4, 0],
        )?;
        let separate_linear_pipeline = raw_texture_pipeline_at(
            &window,
            SEPARATE_NORMALIZED_FRAGMENT_SHADER,
            [6.0, 0.0],
            [6, 0],
        )?;
        let combined_nearest_resources = raw::Resources::builder(&combined_nearest_pipeline)
            .sampled_texture(0, 0, &texture, &nearest_sampler)
            .build(&context)?;
        let combined_linear_resources = raw::Resources::builder(&combined_linear_pipeline)
            .sampled_texture(0, 0, &texture, &linear_sampler)
            .build(&context)?;
        let separate_nearest_resources = raw::Resources::builder(&separate_nearest_pipeline)
            .texture(0, 0, &texture)
            .sampler(1, 0, &nearest_sampler)
            .build(&context)?;
        let separate_linear_resources = raw::Resources::builder(&separate_linear_pipeline)
            .texture(0, 0, &texture)
            .sampler(1, 0, &linear_sampler)
            .build(&context)?;
        let geometry =
            raw::Geometry::builder(textured_quad_at_uv([0.375, 0.375])).build(&context)?;

        window
            .render()
            .draw_raw_2d_with(
                &combined_nearest_pipeline,
                &combined_nearest_resources,
                [&geometry],
            )
            .draw_raw_2d_with(
                &combined_linear_pipeline,
                &combined_linear_resources,
                [&geometry],
            )
            .draw_raw_2d_with(
                &separate_nearest_pipeline,
                &separate_nearest_resources,
                [&geometry],
            )
            .draw_raw_2d_with(
                &separate_linear_pipeline,
                &separate_linear_resources,
                [&geometry],
            )
            .submit()?;

        let readback = readback_first_2x2(&mut window, target);
        for x_offset in [0, 16] {
            assert_eq!(&readback[x_offset..x_offset + 4], &[255, 0, 0, 255]);
        }
        // At (0.375, 0.375), bilinear weights produce (0.625, 0.25, 0.25, 1.0).
        // Allow one UNORM step for implementation rounding while checking both layouts.
        for x_offset in [8, 24] {
            let observed = &readback[x_offset..x_offset + 4];
            let expected = [159_u8, 64, 64, 255];
            for (channel, (actual, expected)) in observed.iter().zip(expected).enumerate() {
                assert!(
                    actual.abs_diff(expected) <= 1,
                    "linear sample channel {channel}: expected {expected} ±1, got {actual}"
                );
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_depth_gather_rejects_both_sampler_coordinate_modes() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let window = Window::builder().build(&context)?;
        let texture = raw_test_texture(&context)?;
        let combined = raw_texture_pipeline(&window, COMBINED_DEPTH_GATHER_SHADER)?;
        let separate = raw_texture_pipeline(&window, SEPARATE_DEPTH_GATHER_SHADER)?;
        for config in [
            SamplerConfig::nearest(),
            SamplerConfig::nearest()
                .with_coordinate_mode(SamplerCoordinateMode::Texel)
                .with_max_lod(SamplerLodLimit::Value(0.0)),
        ] {
            let sampler = Sampler::builder(&context).config(config).build()?;
            assert_invalid_state_contains(
                raw::Resources::builder(&combined)
                    .sampled_texture(0, 0, &texture, &sampler)
                    .build(&context),
                "set 0 binding 0 requires depth comparison",
            );
            assert_invalid_state_contains(
                raw::Resources::builder(&separate)
                    .texture(0, 0, &texture)
                    .sampler(1, 0, &sampler)
                    .build(&context),
                "set 1 binding 0 requires depth comparison",
            );
        }

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_texel_sampler_requires_explicit_static_zero_lod() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().build(&context)?;
        let Some(target) = install_rgba8_readback_target(&mut window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };
        let texture = raw_test_texture(&context)?;
        let texel_pipeline = raw_texture_pipeline(&window, SEPARATE_TEXEL_FRAGMENT_SHADER)?;
        let texel_sampler = Sampler::builder(&context)
            .config(
                SamplerConfig::nearest()
                    .with_coordinate_mode(SamplerCoordinateMode::Texel)
                    .with_max_lod(SamplerLodLimit::Value(0.0)),
            )
            .build()?;
        let texel_resources = raw::Resources::builder(&texel_pipeline)
            .texture(0, 0, &texture)
            .sampler(1, 0, &texel_sampler)
            .build(&context)?;
        let texel_geometry =
            raw::Geometry::builder(textured_quad_at_uv([0.5, 0.5])).build(&context)?;

        let implicit_pipeline = raw_texture_pipeline(&window, SEPARATE_IMPLICIT_FRAGMENT_SHADER)?;
        assert_invalid_state_contains(
            raw::Resources::builder(&implicit_pipeline)
                .texture(0, 0, &texture)
                .sampler(1, 0, &texel_sampler)
                .build(&context),
            "incompatible with texel coordinates",
        );

        let nonzero_lod_pipeline =
            raw_texture_pipeline(&window, SEPARATE_NONZERO_LOD_FRAGMENT_SHADER)?;
        assert_invalid_state_contains(
            raw::Resources::builder(&nonzero_lod_pipeline)
                .texture(0, 0, &texture)
                .sampler(1, 0, &texel_sampler)
                .build(&context),
            "explicit LOD zero",
        );

        window
            .render()
            .draw_raw_2d_with(&texel_pipeline, &texel_resources, [&texel_geometry])
            .submit()?;
        let readback = readback_first_2x2(&mut window, target);
        assert_eq!(&readback[0..4], &[255, 0, 0, 255]);

        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_frame_uniform_texture_upload_is_shared_by_two_windows() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut first_window = Window::builder().build(&context)?;
        let mut second_window = Window::builder().build(&context)?;
        let Some(first_target) = install_rgba8_readback_target(&mut first_window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };
        let Some(second_target) = install_rgba8_readback_target(&mut second_window) else {
            log::info!("R8G8B8A8_UNORM color+transfer target is unsupported; skipped");
            return Ok(());
        };

        let first_pipeline =
            raw_texture_pipeline(&first_window, TEXTURED_FRAME_UNIFORM_FRAGMENT_SHADER)?;
        let second_pipeline =
            raw_texture_pipeline(&second_window, TEXTURED_FRAME_UNIFORM_FRAGMENT_SHADER)?;
        let mut first_uniform =
            raw::FrameUniform::builder(TextureTint { tint: [1.0; 4] }).build(&first_window)?;
        let mut second_uniform =
            raw::FrameUniform::builder(TextureTint { tint: [1.0; 4] }).build(&second_window)?;

        // Keep the transfer observable as pending while both consumers are prepared.
        let texel_count = usize::try_from(PENDING_UPLOAD_EXTENT)
            .expect("upload extent should fit usize")
            .checked_mul(
                usize::try_from(PENDING_UPLOAD_EXTENT).expect("upload extent should fit usize"),
            )
            .expect("upload texel count should fit usize");
        let pixels = [255_u8, 0, 0, 255].repeat(texel_count);
        let texture = Texture2D::from_pixels(
            &context,
            PixelSource::new(
                PENDING_UPLOAD_EXTENT,
                PENDING_UPLOAD_EXTENT,
                TextureFormat::Rgba8Unorm,
                &pixels,
            )?,
        )
        .prepare()?
        .submit()?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let first_resources = raw::Resources::builder(&first_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .frame_uniform(0, 1, &first_uniform)
            .build(&context)?;
        let second_resources = raw::Resources::builder(&second_pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .frame_uniform(0, 1, &second_uniform)
            .build(&context)?;
        assert_eq!(
            texture.initial_upload().wait(Some(Duration::ZERO))?,
            UploadWaitOutcome::TimedOut,
            "the two-window scenario requires an upload confirmed pending before submission",
        );

        let geometry = raw::Geometry::builder(textured_quad_at_uv([0.5, 0.5])).build(&context)?;
        first_window
            .render()
            .write_frame_uniform(
                &mut first_uniform,
                TextureTint {
                    tint: [0.5, 1.0, 1.0, 1.0],
                },
            )
            .draw_raw_2d_with(&first_pipeline, &first_resources, [&geometry])
            .submit()?;
        second_window
            .render()
            .write_frame_uniform(
                &mut second_uniform,
                TextureTint {
                    tint: [0.25, 1.0, 1.0, 1.0],
                },
            )
            .draw_raw_2d_with(&second_pipeline, &second_resources, [&geometry])
            .submit()?;

        // Both frames are submitted before either readback waits for completion.
        let first_readback = readback_first_2x2(&mut first_window, first_target);
        let second_readback = readback_first_2x2(&mut second_window, second_target);
        let first_row_stride =
            usize::try_from(first_window.inner.handle.swapchain.image_extent()[0])
                .expect("target width should fit usize")
                .checked_mul(4)
                .expect("row stride should fit usize");
        let second_row_stride =
            usize::try_from(second_window.inner.handle.swapchain.image_extent()[0])
                .expect("target width should fit usize")
                .checked_mul(4)
                .expect("row stride should fit usize");
        assert_first_2x2_color(&first_readback, first_row_stride, [128, 0, 0, 255]);
        assert_first_2x2_color(&second_readback, second_row_stride, [64, 0, 0, 255]);

        Ok(())
    }
}
