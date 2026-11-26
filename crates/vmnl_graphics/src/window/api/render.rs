// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Public rendering entry points for `Window`.

use crate::d2::Drawable2D;
use crate::raw::{
    BufferContents as RawBufferContents, FrameUniform as RawFrameUniform, Geometry as RawGeometry,
    PendingFrameUniformWrite, Pipeline as RawPipeline, Resources as RawResources,
};
use crate::window::render::RenderPassCommand;
use crate::window::Window;
use crate::VMNLResult;

/// Draw submission strategy for objects inside each logical render pass.
///
/// The mode may change how objects are submitted inside one pass, but it must
/// not reorder calls to [`FrameRenderer::draw2d`],
/// [`FrameRenderer::draw_raw_2d`], or [`FrameRenderer::draw_raw_2d_with`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RenderMode {
    /// Submit each object independently.
    #[default]
    PerObject,
    /// Batch compatible objects when a backend supports it.
    ///
    /// This currently falls back to [`RenderMode::PerObject`].
    Batched,
}

/// Pending frame render operation created by [`Window::render`].
///
/// `FrameRenderer` records the logical passes requested for the next frame.
/// Passes are submitted in the order they are added, including 2D and raw
/// passes. No swapchain image is acquired and no GPU command is submitted until
/// [`FrameRenderer::submit`] is called.
pub struct FrameRenderer<'w, 'g> {
    window: &'w mut Window,
    mode: RenderMode,
    frame_uniform_writes: Vec<PendingFrameUniformWrite<'g>>,
    passes: Vec<RenderPassCommand>,
}

impl<'w, 'g> FrameRenderer<'w, 'g> {
    pub(crate) fn new(window: &'w mut Window) -> Self {
        Self {
            window,
            mode: RenderMode::default(),
            frame_uniform_writes: Vec::new(),
            passes: Vec::new(),
        }
    }

    /// Sets the draw submission strategy used by this frame.
    ///
    /// The mode applies to the objects inside each pass. It does not change the
    /// order of the passes added with [`FrameRenderer::draw2d`],
    /// [`FrameRenderer::draw_raw_2d`], or [`FrameRenderer::draw_raw_2d_with`].
    ///
    /// # Arguments
    ///
    /// - `mode`: Submission strategy used for objects inside each pass. See [`RenderMode`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use vmnl_graphics::{Context, RenderMode, Window};
    /// # use vmnl_graphics::d2::Shape;
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// # let context = Context::new()?;
    /// # let mut window = Window::builder().build(&context)?;
    /// # let rect = Shape::rect(100.0, 100.0).build(&context)?;
    /// window.render()
    ///     .mode(RenderMode::PerObject)
    ///     .draw2d([&rect])
    ///     .submit()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn mode(mut self, mode: RenderMode) -> Self {
        self.mode = mode;
        self
    }

    /// Adds a 2D render pass to the pending frame.
    ///
    /// Each item must implement [`Drawable2D`]. The pass is rendered in the
    /// same logical order as it was added when [`FrameRenderer::submit`] runs.
    ///
    /// # Arguments
    ///
    /// - `drawables`: Fixed-size array of 2D drawable references to render in this pass.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use vmnl_graphics::{Context, Window};
    /// # use vmnl_graphics::d2::Shape;
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// # let context = Context::new()?;
    /// # let mut window = Window::builder().build(&context)?;
    /// let rect = Shape::rect(100.0, 100.0).build(&context)?;
    ///
    /// window.render()
    ///     .draw2d([&rect])
    ///     .submit()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn draw2d<D, const N: usize>(mut self, drawables: [&'g D; N]) -> Self
    where
        D: Drawable2D + ?Sized,
    {
        self.passes.push(RenderPassCommand::D2 {
            items: drawables
                .into_iter()
                .map(Drawable2D::render_item_2d)
                .collect(),
        });
        self
    }

    /// Adds a raw render pass to the pending frame.
    ///
    /// Each draw resolves and applies the pipeline's viewport/scissor policies in
    /// framebuffer pixels against the acquired image after any recreation. Fixed
    /// rectangles keep their values across resize; full policies follow the image.
    /// Subsequent raw draws apply their own states, and 2D passes restore their
    /// full viewport and unclipped pipeline state.
    ///
    /// # Arguments
    ///
    /// - `pipeline` supplies the raw pipeline from [`crate::raw::PipelineSpec::build`] for this
    ///   frame's window. Use [`crate::raw::Pipeline::validate_target`] to check the target.
    /// - `geometries` supplies meshes built on the window's context, in draw order, with the
    ///   vertex layout expected by `pipeline`.
    ///
    /// # Example
    ///
    /// <a id="example-draw-raw-2d"></a>
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Window, VMNLResult};
    /// use vmnl_graphics::raw::{BufferContents, FrameUniform, Geometry, Pipeline, Resources};
    ///
    /// // Build the pipeline for this window and the geometry on the same context.
    /// fn draw<V>(window: &mut Window, pipeline: &Pipeline<V>, geometry: &Geometry<V>) -> VMNLResult<()> {
    ///     window.render().draw_raw_2d(pipeline, [geometry]).submit()
    /// }
    ///
    /// // Match the resource bindings and uniform layout to the pipeline's shaders.
    /// fn draw_with_color<V, D: BufferContents>(
    ///     window: &mut Window,
    ///     pipeline: &Pipeline<V>,
    ///     geometry: &Geometry<V>,
    ///     resources: &Resources,
    ///     uniform: &mut FrameUniform<D>,
    ///     value: D,
    /// ) -> VMNLResult<()> {
    ///     window.render()
    ///         .write_frame_uniform(uniform, value)
    ///         .draw_raw_2d_with(pipeline, resources, [geometry])
    ///         .submit()
    /// }
    /// ```
    #[must_use]
    pub fn draw_raw_2d<TVertex, const N: usize>(
        mut self,
        pipeline: &'g RawPipeline<TVertex>,
        geometries: [&'g RawGeometry<TVertex>; N],
    ) -> Self {
        self.passes.push(RenderPassCommand::Raw {
            items: geometries
                .into_iter()
                .map(|geometry| pipeline.render_item(geometry))
                .collect(),
        });
        self
    }

    /// Adds a raw render pass with descriptor resources to the pending frame.
    ///
    /// Viewport/scissor resolution and isolation follow [`Self::draw_raw_2d`].
    ///
    /// # Arguments
    ///
    /// - `pipeline` supplies the raw pipeline built for this frame's window. Use
    ///   [`crate::raw::Pipeline::validate_target`] to check the target.
    /// - `resources` supplies descriptor bindings built from [`crate::raw::Resources::builder`]
    ///   using this pipeline and its context.
    /// - `geometries` supplies meshes built on the window's context, in draw order, with the vertex
    ///   layout expected by `pipeline`.
    ///
    /// # Example
    ///
    /// See [`draw_raw_2d_with` in the shared example](Self#example-draw-raw-2d).
    #[must_use]
    pub fn draw_raw_2d_with<TVertex, const N: usize>(
        mut self,
        pipeline: &'g RawPipeline<TVertex>,
        resources: &'g RawResources,
        geometries: [&'g RawGeometry<TVertex>; N],
    ) -> Self {
        self.passes.push(RenderPassCommand::Raw {
            items: geometries
                .into_iter()
                .map(|geometry| pipeline.render_item_with(geometry, resources))
                .collect(),
        });
        self
    }

    /// Queues a frame-uniform write for the pending submission.
    ///
    /// The write is applied during [`FrameRenderer::submit`], after VMNL
    /// acquires the swapchain image and before it records the command buffer.
    /// This replaces the uniform buffer slot matching the acquired image with
    /// a freshly allocated or reused subbuffer, then descriptor sets for
    /// frame-uniform resources are created during command recording.
    /// Frame-uniform writes are not ordered between draw passes; all queued
    /// writes are applied before any pass is recorded.
    ///
    /// [`FrameRenderer::submit`] checks the swapchain image count before acquisition and
    /// reports any allocation, binding, or write failure encountered during submission.
    ///
    /// # Arguments
    ///
    /// - `uniform`: Frame-uniform to update for the acquired swapchain image.
    /// - `data`: New shader-visible value for this frame.
    ///
    /// # Example
    ///
    /// See [`write_frame_uniform` in the shared example](Self#example-draw-raw-2d).
    #[must_use]
    pub fn write_frame_uniform<TData>(
        mut self,
        uniform: &'g mut RawFrameUniform<TData>,
        data: TData,
    ) -> Self
    where
        TData: RawBufferContents + 'g,
    {
        self.frame_uniform_writes
            .push(PendingFrameUniformWrite::new(uniform, data));
        self
    }

    /// Submits the frame to the GPU.
    ///
    /// The recorded passes are consumed by this call. A frame with no draw pass
    /// is valid and still clears/presents the swapchain image.
    ///
    /// After a logical or framebuffer resize, `submit` recreates the
    /// swapchain images and framebuffers before recording the next frame. It resolves the
    /// extent, surface transform and composite alpha from fresh capabilities. It retains the
    /// requested minimum image count while it remains valid. The driver can still return a
    /// different actual image count. A zero-sized framebuffer or surface, for example while
    /// minimized, returns `VMNLErrorKind::VulkanOutOfDate` until it has a renderable extent again.
    ///
    /// Returns `Ok(())` when command recording, queue submission, presentation,
    /// and optional event polling complete without error. This does not guarantee
    /// that the displayed frame was visible to the user.
    ///
    /// # Errors
    ///
    /// Raw pipeline, geometry, and resource compatibility, raw viewport/scissor
    /// validity, and frame-uniform image counts are checked before acquisition.
    /// If acquisition requires swapchain recreation, these checks run again for
    /// the replacement swapchain before retrying. If the actual image count changes, rebuild
    /// the affected raw `FrameUniform` values and the `Resources` that bind them.
    ///
    /// Returns `TextureUploadFailed` before acquiring a swapchain image if an initial
    /// upload used by a raw draw has a known failure or its fence status query fails.
    /// Returns an error if frame-uniform allocation/write/binding, frame acquisition,
    /// command recording, submission, or presentation fails. An out-of-date swapchain
    /// during acquisition or presentation returns `VMNLErrorKind::VulkanOutOfDate`.
    /// Presentation can report this after work has been submitted; the error does not
    /// mean that no work was submitted or that its resources are idle. VMNL preserves
    /// the required synchronization and requests swapchain recreation, so the same
    /// window can be used again after its framebuffer is renderable.
    /// Recreation preserves the window's color pair. `SurfaceColorPairLost` means the surface
    /// no longer reports it. Retry if the pair returns, or build a new window with a different
    /// policy. A failed framebuffer allocation can be retried with the replacement swapchain.
    /// `SwapchainRecreationRequiresNewWindow` means a previous native replacement failed after
    /// retiring the old swapchain and the window must be rebuilt.
    /// Returns `VMNLErrorKind::VulkanDeviceLost` if presentation or the subsequent fence
    /// submission reports device loss. Error cleanup can block the calling thread while
    /// the queue retires submitted resources.
    ///
    /// Earlier `0.1.0` builds logged a presentation-stage out-of-date result and
    /// returned `Ok(())`. Callers that relied on that behavior must now handle
    /// `VulkanOutOfDate` as a transient submission error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use vmnl_graphics::{Context, RenderMode, Window};
    /// # use vmnl_graphics::common::Rgba;
    /// # use vmnl_graphics::d2::Shape;
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// # let context = Context::new()?;
    /// # let mut window = Window::builder().build(&context)?;
    /// let rectangle = Shape::rect(200.0, 100.0)
    ///     .position(100.0, 150.0)
    ///     .color(Rgba::new(255, 0, 0, 255))
    ///     .build(&context)?;
    ///
    /// window.render()
    ///     .mode(RenderMode::PerObject)
    ///     .draw2d([&rectangle])
    ///     .submit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit(self) -> VMNLResult<()> {
        self.window
            .inner
            .render_commands(self.mode, &self.passes, self.frame_uniform_writes)
    }
}

impl Window {
    /// Begins a new pending frame render operation.
    ///
    /// Use the returned [`FrameRenderer`] to append 2D or raw passes,
    /// then call [`FrameRenderer::submit`] to render and present the frame.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use vmnl_graphics::{Context, Window};
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// # let context = Context::new()?;
    /// # let mut window = Window::builder().build(&context)?;
    /// window.render().submit()?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn render(&mut self) -> FrameRenderer<'_, '_> {
        FrameRenderer::new(self)
    }
}
