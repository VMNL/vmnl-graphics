// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

mod api;
mod builder;
mod event;
mod inner;
mod input;
mod monitors;
mod render;
mod render_target;
mod render_target_info;
mod runtime;
mod shaders;
mod surface_color;
use std::sync::Arc;

use crate::window::inner::VMNLWindow;
use crate::{Context, VMNLResult};
pub use api::{FrameRenderer, RenderMode};
pub(crate) use builder::{validate_size_limits, validate_window_size, WindowOptions};
pub use builder::{PresentMode, WindowBuilder};
pub use event::{Event, EventKind};
pub(crate) use input::CursorImage;
pub use input::{
    Cursor, CursorBuilder, CursorMode, GamepadAxis, GamepadButton, GamepadState, Input,
    JoystickButtonState, JoystickHatState, JoystickId, JoystickSample, JoystickState,
    JoystickStatus, Key, KeyboardState, Modifiers, MouseButton, MouseState, Scancode,
    StandardCursor, StandardCursorBuilder, StickAngleConvention, StickConfig, StickState,
};
pub use monitors::{MonitorInfo, Monitors, VideoMode};
pub use render_target::{DepthFormat, RenderTargetConfig};
pub use render_target_info::{ColorFormat, RenderTargetInfo};
pub use surface_color::{
    SurfaceColorConfig, SurfaceColorFallback, SurfaceColorInfo, SurfaceColorPair,
    SurfaceColorSelection, SurfaceColorSpace,
};
use vulkano::{
    device::Device, memory::allocator::StandardMemoryAllocator, render_pass::RenderPass,
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    /// Current size of the window used for scaling and coordinate transformations in shaders.
    window_size: [f32; 2],
}

/// The `Window` struct represents an application window in the VMNL library, providing methods for managing window properties, handling events, and coordinating rendering.
///
/// It serves as the primary interface for interacting with the windowing system and encapsulates the underlying implementation details.
pub struct Window {
    /// The internal implementation of the window, which manages the actual GLFW window and related state.
    pub(in crate::window) inner: VMNLWindow,
}

impl Window {
    /// Creates a new `Window` instance with default configuration options.
    ///
    /// Returns a `VMNLResult` containing the newly created `Window` instance or an error if the window creation fails.
    ///
    /// # Arguments
    ///
    /// - `context`: The graphics context to associate with the window.
    ///
    /// # Errors
    ///
    /// Propagates the same initialization errors as [`WindowBuilder::build`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, Window};
    ///
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let window = Window::new(&context)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(context: &Context) -> VMNLResult<Self> {
        Self::builder().build(context)
    }

    /// Provides a builder for constructing a `Window` instance with customizable options.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, Window};
    ///
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let window = Window::builder()
    ///     .title("VMNL")
    ///     .size(800, 600)
    ///     .build(&context)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn builder() -> WindowBuilder {
        WindowBuilder::default()
    }

    /// Internal method to create a `Window` instance from the provided `WindowOptions`.
    pub(crate) fn from_options(context: &Context, options: &WindowOptions) -> VMNLResult<Self> {
        options.render_target.validate()?;
        validate_window_size(options.width, options.height)?;
        validate_size_limits(
            options.min_width,
            options.min_height,
            options.max_width,
            options.max_height,
        )?;
        let mut inner_window: VMNLWindow = VMNLWindow::create(context, options)?;

        if options.configure_window_polling {
            inner_window.configure_window_polling();
        }
        inner_window.set_size_limits(
            options.min_width,
            options.min_height,
            options.max_width,
            options.max_height,
        )?;
        inner_window.set_clear_color(options.clear_color);
        Ok(Self {
            inner: inner_window,
        })
    }

    /// Returns the immutable attachment configuration validated at window construction.
    ///
    /// The returned copy performs no allocation, synchronization or GPU query. Depth
    /// format and clear value persist across resize; image extent follows the swapchain.
    /// This does not expose backend image handles or imply that depth testing is enabled.
    ///
    /// # Example
    ///
    /// See [`render_target_config` in the shared example](Self#example-window-render-target).
    #[must_use]
    pub const fn render_target_config(&self) -> RenderTargetConfig {
        self.inner.config.render_target
    }

    /// Returns resolved attachment properties, copied from the created render pass.
    ///
    /// Color format includes the surface fallback selected at build; samples and
    /// depth format describe actual attachments. Resize preserves this description.
    /// This allocates nothing and performs no GPU query or synchronization. Equal
    /// descriptions do not make pipelines shareable across windows; use
    /// [`crate::raw::Pipeline::validate_target`] for that check.
    ///
    /// # Example
    ///
    /// <a id="example-window-render-target"></a>
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, Window};
    ///
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let window = Window::new(&context)?;
    /// let config = window.render_target_config();
    /// let info = window.render_target_info();
    /// assert_eq!(info.depth_format(), config.depth_value());
    /// assert_eq!(info.sample_count(), 1);
    /// println!("Actual swapchain color format {}", info.color_format());
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub const fn render_target_info(&self) -> RenderTargetInfo {
        self.inner.config.render_target_info
    }

    /// Borrows the surface color request and resolved pair captured at window creation.
    ///
    /// Normal swapchain recreation preserves this pair. This borrows the snapshot without
    /// allocating, querying the surface or synchronizing with the GPU.
    ///
    /// <a id="example-window-surface-color"></a>
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{ColorFormat, Context, SurfaceColorConfig, SurfaceColorSelection,
    ///     SurfaceColorSpace, Window};
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let window = Window::builder()
    ///     .surface_color(SurfaceColorConfig::exact(
    ///         ColorFormat::B8G8R8A8_SRGB, SurfaceColorSpace::SRGB_NON_LINEAR,
    ///     ))
    ///     .build(&context)?;
    /// let info = window.surface_color_info();
    /// assert_eq!(info.pair().format(), ColorFormat::B8G8R8A8_SRGB);
    /// assert_eq!(info.requested().exact_pair(), Some(info.pair()));
    /// assert_eq!(info.selection(), SurfaceColorSelection::Exact);
    /// assert!(!info.used_fallback());
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub const fn surface_color_info(&self) -> &SurfaceColorInfo {
        &self.inner.config.surface_color_info
    }

    /// Queries the current format and color-space pairs on this window's surface.
    ///
    /// Call on the window thread. This performs a native query and allocates a vector, preserving
    /// the report order of pairs representable by the backend with an enabled presentation color
    /// space. A new window must
    /// validate its own surface at construction. A reported pair can still fail render-resource
    /// creation. The presentation pair is independent of source color conversion and blending.
    ///
    /// # Errors
    ///
    /// Returns `VulkanSurfaceColorQueryFailed` if the format query fails, or `VulkanSurfaceLost`
    /// or `VulkanOutOfMemory` when reported by Vulkan.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # fn inspect(window: &vmnl_graphics::Window) -> vmnl_graphics::VMNLResult<()> {
    /// for pair in window.surface_color_pairs()? {
    ///     println!("{} / {}", pair.format(), pair.color_space());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn surface_color_pairs(&self) -> VMNLResult<Vec<SurfaceColorPair>> {
        self.inner.surface_color_pairs()
    }

    pub(crate) fn device(&self) -> Arc<Device> {
        self.inner.handle.vmnl_instance.device.clone()
    }

    pub(crate) fn raw_color_blend_defaults(&self) -> crate::common::ColorBlendOverrides {
        self.inner.handle.vmnl_instance.raw_color_blend_defaults
    }

    pub(crate) fn memory_allocator(&self) -> Arc<StandardMemoryAllocator> {
        self.inner.handle.vmnl_instance.memory_allocator.clone()
    }

    pub(crate) fn render_pass(&self) -> Arc<RenderPass> {
        self.inner.handle.render_pass.clone()
    }

    pub(crate) fn swapchain_image_count(&self) -> usize {
        self.inner.handle.framebuffers.len()
    }

    pub(crate) fn render_extent(&self) -> [u32; 2] {
        self.inner.handle.swapchain.image_extent()
    }
}
