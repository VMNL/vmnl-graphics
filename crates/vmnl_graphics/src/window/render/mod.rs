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
        if self.state.swapchain_recreation_requested {
            self.recreate_swapchain()?;
        }
        Self::begin_frame(&mut self.handle.previous_frame_end);
        let (image_index, suboptimal, acquire_future): (u32, bool, SwapchainAcquireFuture) =
            match Self::acquire_next_image_from_swapchain(&self.handle.swapchain, None) {
                Ok(acquisition) => acquisition,
                Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate) => {
                    self.recreate_swapchain()?;
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
        let current_image_count = self.handle.framebuffers.len();
        for write in frame_uniform_writes {
            write.write(framebuffer_index, current_image_count)?;
        }

        let command_buffer: Arc<PrimaryAutoCommandBuffer> =
            self.build_command_buffer(image_index, commands)?;
        let upload_dependencies = commands
            .iter()
            .filter_map(|command| match command {
                RenderPassCommand::Raw { items } => Some(items),
                RenderPassCommand::D2 { .. } => None,
            })
            .flat_map(|items| items.iter().flat_map(RenderItemRaw::upload_dependencies))
            .collect::<Vec<TextureUploadDependency>>();
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

    use crate::raw::{self, Scissor, ScissorPolicy, Viewport, ViewportPolicy};
    use crate::texture::{PixelSource, Sampler, SamplerConfig, Texture2D, TextureFormat};
    use crate::{Context, VMNLErrorKind, VMNLResult, Window};
    use std::sync::Arc;
    use vulkano::{
        buffer::{Buffer, BufferCreateInfo, BufferUsage},
        command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo},
        format::Format,
        image::{
            view::ImageView, Image, ImageCreateInfo, ImageFormatInfo, ImageLayout, ImageUsage,
            SampleCounts,
        },
        memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
        render_pass::{Framebuffer, FramebufferCreateInfo},
        sync::{self, GpuFuture},
    };

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

    const UNIFORM_FRAGMENT_SHADER: &str = r"
        #version 450
        layout(set = 0, binding = 0) uniform Tint { vec4 tint; };
        layout(location = 0) in vec2 texture_uv;
        layout(location = 0) out vec4 out_color;
        void main() {
            out_color = tint;
        }
    ";

    #[repr(C)]
    #[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
    struct TexturedVertex {
        #[format(R32G32_SFLOAT)]
        position: [f32; 2],
        #[format(R32G32_SFLOAT)]
        uv: [f32; 2],
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
}
