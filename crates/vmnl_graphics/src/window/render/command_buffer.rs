// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Command buffer submodule for recording Vulkan draw commands.
//!
//! This module builds primary command buffers for rendering VMNL graphics objects
//! into the current swapchain framebuffer.

use crate::common::{BlendMode, MaterialKey, PipelineKey};
use crate::raw::{pipeline_targets_window, resolve_viewport_scissor, PendingFrameUniformWrite};
use crate::window::render::{raw_draw_items, RenderPassCommand};
use crate::window::PushConstants;
use crate::{window::inner::VMNLWindow, VMNLError, VMNLErrorKind, VMNLResult};
use smallvec::smallvec;
use std::sync::Arc;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        RenderPassBeginInfo, SubpassBeginInfo, SubpassContents, SubpassEndInfo,
    },
    pipeline::Pipeline,
    pipeline::{
        graphics::viewport::{Scissor, Viewport},
        GraphicsPipeline, PipelineBindPoint,
    },
    render_pass::Framebuffer,
    swapchain::Swapchain,
};

impl VMNLWindow {
    /// Validates raw draw bindings and resolves viewport/scissor pairs before image acquisition.
    ///
    /// Each pair corresponds to a raw draw item in submission order. Run again after swapchain
    /// recreation to resolve against the replacement extent.
    pub(super) fn preflight_frame(
        &self,
        commands: &[RenderPassCommand],
        frame_uniform_writes: &[PendingFrameUniformWrite<'_>],
    ) -> VMNLResult<Vec<(Viewport, Scissor)>> {
        let image_count = self.handle.framebuffers.len();
        for write in frame_uniform_writes {
            write.validate_image_count(image_count)?;
        }
        let device = &self.handle.vmnl_instance.device;
        let extent = self.handle.swapchain.image_extent();
        raw_draw_items(commands)
            .map(|item| {
                if !pipeline_targets_window(
                    &item.pipeline_device,
                    &item.pipeline_render_pass,
                    device,
                    &self.handle.render_pass,
                ) || !Arc::ptr_eq(&item.geometry_device, device)
                {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                        "raw pipeline and geometry must belong to this window context".into(),
                    )));
                }
                if item
                    .resources_device
                    .as_ref()
                    .is_some_and(|resource_device| !Arc::ptr_eq(resource_device, device))
                {
                    return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                        "raw resources must belong to this window context".into(),
                    )));
                }
                match &item.resources_pipeline_layout {
                    Some(layout)
                        if item.required_descriptor_set_count > 0
                            && !Arc::ptr_eq(layout, item.pipeline.layout()) =>
                    {
                        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                            "raw resources must be built for this raw pipeline".into(),
                        )));
                    }
                    None if item.required_descriptor_set_count > 0 => {
                        return Err(VMNLError::new(VMNLErrorKind::InvalidState(
                            "raw pipeline requires descriptor resources".into(),
                        )));
                    }
                    _ => {}
                }
                item.validate_descriptor_sets_for_frame(image_count)?;
                resolve_viewport_scissor(item.viewport, item.scissor, extent, device)
            })
            .collect()
    }

    #[allow(clippy::cast_precision_loss)]
    fn extent_as_f32(extent: [u32; 2]) -> [f32; 2] {
        [extent[0] as f32, extent[1] as f32]
    }

    /// Builds a Vulkan command buffer for rendering the provided graphics objects to the specified swapchain image.
    ///
    /// Returns an `Arc<PrimaryAutoCommandBuffer>` containing the built command buffer ready for execution.
    ///
    /// # Arguments
    /// - `image_index`: Index of the swapchain image to render to.
    /// - `commands`: Ordered render commands to record.
    /// - `raw_draw_states`: Viewport and scissor resolved in raw draw order for this swapchain.
    #[allow(clippy::too_many_lines)]
    pub(super) fn build_command_buffer(
        &self,
        image_index: u32,
        commands: &[RenderPassCommand],
        raw_draw_states: Vec<(Viewport, Scissor)>,
    ) -> VMNLResult<Arc<PrimaryAutoCommandBuffer>> {
        let mut raw_draw_states = raw_draw_states.into_iter();
        let swapchain: &Arc<Swapchain> = &self.handle.swapchain;
        let allocator: Arc<StandardCommandBufferAllocator> =
            self.handle.vmnl_instance.command_buffer_allocator.clone();
        let queue_family: u32 = self
            .handle
            .vmnl_instance
            .graphics_queue
            .queue_family_index();
        let extent: [u32; 2] = swapchain.image_extent();
        let extent_f32: [f32; 2] = Self::extent_as_f32(extent);
        let framebuffer_index: usize = usize::try_from(image_index)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        let framebuffer: Arc<Framebuffer> = self
            .handle
            .framebuffers
            .get(framebuffer_index)
            .cloned()
            .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        let mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> =
            AutoCommandBufferBuilder::primary(
                allocator,
                queue_family,
                CommandBufferUsage::OneTimeSubmit,
            )
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanCommandBufferCreationFailed))?;

        // SAFETY: all resources belong to this window's Vulkan context, remain owned while the
        // command buffer exists, and Vulkano validates each command before recording it.
        // Preflight checks raw viewport/device bounds and signed scissor sums for this swapchain;
        // both dynamic states are set after each raw pipeline bind, before any draw.
        // Optional depth images are owned by their framebuffer and match its extent;
        // the clear list follows the render pass's color/depth attachment order.
        unsafe {
            builder
                .begin_render_pass(
                    RenderPassBeginInfo {
                        clear_values: {
                            let mut values = vec![Some(self.state.clear_color.into())];
                            if self.config.render_target.depth_value().is_some() {
                                values.push(Some(
                                    self.config.render_target.clear_depth_value().into(),
                                ));
                            }
                            values
                        },
                        ..RenderPassBeginInfo::framebuffer(framebuffer)
                    },
                    SubpassBeginInfo {
                        contents: SubpassContents::Inline,
                        ..Default::default()
                    },
                )
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanRenderPassCreationFailed))?
                .set_viewport(
                    0,
                    smallvec![Viewport {
                        offset: [0.0, 0.0],
                        extent: extent_f32,
                        depth_range: 0.0..=1.0,
                    }],
                )
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
            let mut current_blend_mode: Option<BlendMode> = None;
            for command in commands {
                match command {
                    RenderPassCommand::D2 { items } => {
                        builder
                            .set_viewport(
                                0,
                                smallvec![Viewport {
                                    offset: [0.0, 0.0],
                                    extent: extent_f32,
                                    depth_range: 0.0..=1.0,
                                }],
                            )
                            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
                        for render_item in items {
                            debug_assert_eq!(render_item.pipeline_key, PipelineKey::Color2D);
                            debug_assert_eq!(render_item.material_key, MaterialKey::VertexColor);
                            let pipeline: &Arc<GraphicsPipeline> = match render_item.blend_mode {
                                BlendMode::Opaque => &self.handle.pipeline_2d_opaque,
                                BlendMode::Alpha => &self.handle.pipeline_2d_alpha,
                            };
                            if current_blend_mode != Some(render_item.blend_mode) {
                                builder
                                    .bind_pipeline_graphics(pipeline.clone())
                                    .map_err(|_| {
                                        VMNLError::new(VMNLErrorKind::VulkanPipelineCreationFailed)
                                    })?;
                                current_blend_mode = Some(render_item.blend_mode);
                            }
                            let push_constants: PushConstants = PushConstants {
                                window_size: extent_f32,
                            };
                            builder
                                .push_constants(pipeline.layout().clone(), 0, push_constants)
                                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?
                                .bind_vertex_buffers(0, render_item.vertex_buffer.as_subbuffer())
                                .map_err(|_| {
                                    VMNLError::new(VMNLErrorKind::VulkanVertexBufferCreationFailed)
                                })?;
                            if let Some(index_buffer) = &render_item.index_buffer {
                                builder
                                    .bind_index_buffer(index_buffer.as_subbuffer())
                                    .map_err(|_| {
                                        VMNLError::new(
                                            VMNLErrorKind::VulkanIndexBufferCreationFailed,
                                        )
                                    })?
                                    .draw_indexed(render_item.index_count, 1, 0, 0, 0)
                                    .map_err(|_| {
                                        VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                                    })?;
                            } else {
                                builder
                                    .draw(render_item.vertex_count, 1, 0, 0)
                                    .map_err(|_| {
                                        VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                                    })?;
                            }
                        }
                    }
                    RenderPassCommand::Raw { items } => {
                        current_blend_mode = None;
                        for render_item in items {
                            let descriptor_sets = render_item.descriptor_sets_for_image(
                                framebuffer_index,
                                &self.handle.vmnl_instance.descriptor_set_allocator,
                                &self.handle.vmnl_instance.device,
                            )?;

                            let (viewport, scissor) = raw_draw_states.next().ok_or_else(|| {
                                VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                            })?;
                            builder
                                .bind_pipeline_graphics(render_item.pipeline.clone())
                                .map_err(|_| {
                                    VMNLError::new(VMNLErrorKind::VulkanPipelineCreationFailed)
                                })?
                                .set_viewport(0, smallvec![viewport])
                                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?
                                .set_scissor(0, smallvec![scissor])
                                .map_err(|_| {
                                    VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                                })?;
                            if !descriptor_sets.is_empty() {
                                builder
                                    .bind_descriptor_sets(
                                        PipelineBindPoint::Graphics,
                                        render_item.pipeline.layout().clone(),
                                        0,
                                        descriptor_sets,
                                    )
                                    .map_err(|_| {
                                        VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                                    })?;
                            }
                            builder
                                .bind_vertex_buffers(0, render_item.vertex_buffer.clone())
                                .map_err(|_| {
                                    VMNLError::new(VMNLErrorKind::VulkanVertexBufferCreationFailed)
                                })?;
                            if let Some(index_buffer) = &render_item.index_buffer {
                                builder
                                    .bind_index_buffer(index_buffer.clone())
                                    .map_err(|_| {
                                        VMNLError::new(
                                            VMNLErrorKind::VulkanIndexBufferCreationFailed,
                                        )
                                    })?
                                    .draw_indexed(render_item.index_count, 1, 0, 0, 0)
                                    .map_err(|_| {
                                        VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                                    })?;
                            } else {
                                builder
                                    .draw(render_item.vertex_count, 1, 0, 0)
                                    .map_err(|_| {
                                        VMNLError::new(VMNLErrorKind::VulkanValidationFailed)
                                    })?;
                            }
                        }
                    }
                }
            }
            builder
                .end_render_pass(SubpassEndInfo::default())
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanRenderPassCreationFailed))?;
        }
        builder
            .build()
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanCommandBufferCreationFailed))
    }
}
