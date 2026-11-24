// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Vulkan swapchain and image-view creation helpers.

use super::VMNLWindow;
use crate::window::WindowOptions;
use crate::{
    ColorFormat, SurfaceColorInfo, SurfaceColorPair, SurfaceColorSpace, VMNLError, VMNLErrorKind,
    VMNLResult,
};
use ash::vk;
use std::sync::Arc;
use vulkano::{
    device::Device,
    format::FormatFeatures,
    image::{
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
        Image, ImageUsage,
    },
    render_pass::Framebuffer,
    swapchain::{
        ColorSpace, Surface, SurfaceCapabilities, SurfaceInfo, Swapchain, SwapchainCreateInfo,
    },
    Validated, VulkanError, VulkanObject,
};

type SwapchainImages = (Arc<Swapchain>, Vec<Arc<Image>>);

/// Tracks replacement progress separately from ordinary resize/out-of-date requests.
#[derive(Default)]
pub(crate) enum SwapchainReplacement {
    #[default]
    Ready,
    /// The current swapchain has never been acquired from. Framebuffers still need to be built.
    PendingFramebuffers(Vec<Arc<Image>>),
    /// Native creation failed after retiring the only swapchain. Its in-flight owners stay alive.
    Retired,
}

impl VMNLWindow {
    /// Creates an image view for each swapchain image.
    ///
    /// Returns a vector of `Arc<ImageView>` corresponding to each swapchain image.
    ///
    /// # Arguments
    /// - `images`: A slice of Vulkan images obtained from the swapchain.
    ///
    /// # Sources
    /// <https://docs.rs/vulkano/latest/vulkano/image/view/index.html>
    pub(super) fn create_image_views(images: &[Arc<Image>]) -> VMNLResult<Vec<Arc<ImageView>>> {
        images
            .iter()
            .map(|image| {
                ImageView::new(
                    image.clone(),
                    ImageViewCreateInfo {
                        view_type: ImageViewType::Dim2d,
                        format: image.format(),
                        subresource_range: image.subresource_range(),
                        ..Default::default()
                    },
                )
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanImageViewCreationFailed))
            })
            .collect()
    }

    /// Creates a Vulkan swapchain for the given surface and device, returning the swapchain and its images.
    ///
    /// Returns the swapchain images and the selection snapshot used to create them.
    ///
    /// # Arguments
    /// - `device`: Vulkan logical device used to create the swapchain and query surface capabilities.
    /// - `surface`: Vulkan surface representing the OS window to present rendered images to.
    /// - `window_extent`: Desired dimensions of the swapchain images, typically matching the window size.
    /// - `options`: Desired surface color and presentation policies.
    ///
    /// # Sources
    /// <https://docs.rs/vulkano/latest/vulkano/swapchain/index.html>
    pub(super) fn create_swapchain(
        device: &Arc<Device>,
        surface: &Arc<Surface>,
        window_extent: [u32; 2],
        options: &WindowOptions,
    ) -> VMNLResult<(SwapchainImages, SurfaceColorInfo)> {
        let surface_capabilities: SurfaceCapabilities = device
            .physical_device()
            .surface_capabilities(surface, SurfaceInfo::default())
            .map_err(|error| surface_error(&error, VMNLErrorKind::VulkanSurfaceCreationFailed))?;
        let surface_pairs = device
            .physical_device()
            .surface_formats(surface, SurfaceInfo::default())
            .map_err(|error| surface_error(&error, VMNLErrorKind::VulkanSurfaceColorQueryFailed))?
            .into_iter()
            .filter(|&(_, space)| surface_color_space_enabled(device, space))
            .map(|(format, space)| {
                SurfaceColorPair::new(ColorFormat(format), SurfaceColorSpace(space))
            })
            .collect::<Vec<_>>();
        let supported_present_modes = device
            .physical_device()
            .surface_present_modes(surface, SurfaceInfo::default())
            .map_err(|error| surface_error(&error, VMNLErrorKind::VulkanSurfaceCreationFailed))?;
        let selected_present_mode = options.present_mode.select_vk(&supported_present_modes)?;
        // Every window creates both 2D variants, including the alpha-blended pipeline.
        let selection = options.surface_color.select(&surface_pairs, |format| {
            let features = device
                .physical_device()
                .format_properties(format.0)
                .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?
                .optimal_tiling_features;
            Ok(features.contains(FormatFeatures::COLOR_ATTACHMENT_BLEND))
        })?;
        let image_format = selection.pair().format().0;
        let image_color_space = selection.pair().color_space().0;
        let mut swapchain_create_info = SwapchainCreateInfo {
            image_format,
            image_color_space,
            image_usage: ImageUsage::COLOR_ATTACHMENT,
            present_mode: selected_present_mode,
            ..Default::default()
        };
        update_surface_options(
            &mut swapchain_create_info,
            window_extent,
            &surface_capabilities,
        )?;
        log::debug!(
            "creating swapchain: extent={}x{}, images={}, surface_color={selection:?}, requested_present_mode={:?}, selected_present_mode={selected_present_mode:?}",
            swapchain_create_info.image_extent[0],
            swapchain_create_info.image_extent[1],
            swapchain_create_info.min_image_count,
            options.present_mode,
        );
        let images = Swapchain::new(device.clone(), surface.clone(), swapchain_create_info)
            .map_err(|error| surface_error(&error, VMNLErrorKind::VulkanSwapchainCreationFailed))?;
        Ok((images, selection))
    }

    /// Requeries surface color pairs. Vulkano caches its typed query for the surface lifetime.
    pub(crate) fn surface_color_pairs(&self) -> VMNLResult<Vec<SurfaceColorPair>> {
        let instance = &self.handle.vmnl_instance.instance;
        let device = &self.handle.vmnl_instance.device;
        let physical_device = self.handle.vmnl_instance.physical_device.handle();
        let surface = self.handle.swapchain.surface().handle();
        let query = instance
            .fns()
            .khr_surface
            .get_physical_device_surface_formats_khr;
        loop {
            let mut count = 0;
            // SAFETY: This window owns a live surface and physical device from the same instance.
            // KHR_surface was enabled for window creation, and this device was checked for
            // presentation support. The null output asks Vulkan to write only the live count.
            unsafe {
                query(
                    physical_device,
                    surface,
                    &raw mut count,
                    std::ptr::null_mut(),
                )
            }
            .result()
            .map_err(surface_color_query_error)?;
            let mut pairs = vec![vk::SurfaceFormatKHR::default(); count as usize];
            // SAFETY: Same live device and surface. The slice has count writable entries,
            // and Vulkan reports INCOMPLETE instead of writing beyond that capacity.
            match unsafe { query(physical_device, surface, &raw mut count, pairs.as_mut_ptr()) } {
                vk::Result::SUCCESS => {
                    pairs.truncate(count as usize);
                    return Ok(pairs
                        .into_iter()
                        .filter_map(|pair| {
                            let format = pair.format.try_into().ok()?;
                            let space = pair.color_space.try_into().ok()?;
                            surface_color_space_enabled(device, space).then_some(
                                SurfaceColorPair::new(
                                    ColorFormat(format),
                                    SurfaceColorSpace(space),
                                ),
                            )
                        })
                        .collect());
                }
                vk::Result::INCOMPLETE => (),
                error => return Err(surface_color_query_error(error)),
            }
        }
    }

    /// Recreates the swapchain images and framebuffers for the current framebuffer extent.
    ///
    /// The render pass and graphics pipelines remain valid because recreation preserves the
    /// existing swapchain format; only resources that reference individual swapchain images are
    /// replaced.
    pub(crate) fn recreate_swapchain(&mut self) -> VMNLResult<()> {
        self.recreate_swapchain_with(Self::build_swapchain_framebuffers)
    }

    fn recreate_swapchain_with(
        &mut self,
        build_framebuffers: impl FnOnce(&Self, &[Arc<Image>]) -> VMNLResult<Vec<Arc<Framebuffer>>>,
    ) -> VMNLResult<()> {
        self.state.swapchain_recreation_requested = true;
        if matches!(
            self.handle.swapchain_replacement,
            SwapchainReplacement::Retired
        ) {
            return Err(VMNLError::new(
                VMNLErrorKind::SwapchainRecreationRequiresNewWindow,
            ));
        }
        let (width, height): (u32, u32) = self.get_framebuffer_size();
        if width == 0 || height == 0 {
            return Err(VMNLError::new(VMNLErrorKind::VulkanOutOfDate));
        }
        let pair = self.config.surface_color_info.pair();
        if !self.surface_color_pairs()?.contains(&pair) {
            return Err(VMNLError::new(VMNLErrorKind::SurfaceColorPairLost { pair }));
        }

        let swapchain = &self.handle.swapchain;
        // Vulkano queries capabilities directly. Only its typed format inventory is cached.
        let capabilities = self
            .handle
            .vmnl_instance
            .physical_device
            .surface_capabilities(swapchain.surface(), SurfaceInfo::default())
            .map_err(|error| surface_error(&error, VMNLErrorKind::VulkanSwapchainCreationFailed))?;
        let mut create_info = swapchain.create_info();
        let surface_options_changed =
            update_surface_options(&mut create_info, [width, height], &capabilities)?;
        if !matches!(
            self.handle.swapchain_replacement,
            SwapchainReplacement::PendingFramebuffers(_)
        ) || surface_options_changed
        {
            self.replace_swapchain_images(create_info)?;
        }

        let SwapchainReplacement::PendingFramebuffers(images) = &self.handle.swapchain_replacement
        else {
            return Err(VMNLError::new(VMNLErrorKind::VulkanSwapchainCreationFailed));
        };
        self.handle.framebuffers = build_framebuffers(self, images)?;
        if self.config.render_target.depth_value().is_some() {
            self.handle.depth_frame_completions = vec![None; images.len()];
        }
        self.handle.swapchain_replacement = SwapchainReplacement::Ready;
        self.state.swapchain_recreation_requested = false;
        Ok(())
    }

    fn replace_swapchain_images(&mut self, create_info: SwapchainCreateInfo) -> VMNLResult<()> {
        match self.handle.swapchain.recreate(create_info) {
            Ok((swapchain, images)) => {
                self.handle.swapchain = swapchain;
                self.handle.swapchain_replacement =
                    SwapchainReplacement::PendingFramebuffers(images);
                Ok(())
            }
            Err(error) => {
                if matches!(error, Validated::Error(_)) {
                    // Vulkano retires the original before calling Vulkan. An error after that
                    // point cannot restore it, and the original may still own submitted work.
                    self.handle.swapchain_replacement = SwapchainReplacement::Retired;
                    self.state.is_ready = false;
                }
                Err(surface_error(
                    &error,
                    VMNLErrorKind::VulkanSwapchainCreationFailed,
                ))
            }
        }
    }

    fn build_swapchain_framebuffers(
        &self,
        images: &[Arc<Image>],
    ) -> VMNLResult<Vec<Arc<Framebuffer>>> {
        let image_views = Self::create_image_views(images)?;
        Self::create_framebuffers(
            &image_views,
            &self.handle.render_pass,
            &self.handle.vmnl_instance.memory_allocator,
            self.config.render_target,
        )
    }
}

/// Resolves automatic parameters against the current surface, leaving explicit choices intact.
/// Returns whether a pending replacement would need a different swapchain.
fn update_surface_options(
    info: &mut SwapchainCreateInfo,
    window_extent: [u32; 2],
    capabilities: &SurfaceCapabilities,
) -> VMNLResult<bool> {
    if window_extent.contains(&0) || capabilities.max_image_extent.contains(&0) {
        return Err(VMNLError::new(VMNLErrorKind::VulkanOutOfDate));
    }
    let extent = capabilities.current_extent.unwrap_or_else(|| {
        std::array::from_fn(|axis| {
            window_extent[axis].clamp(
                capabilities.min_image_extent[axis],
                capabilities.max_image_extent[axis],
            )
        })
    });
    if extent.contains(&0) {
        return Err(VMNLError::new(VMNLErrorKind::VulkanOutOfDate));
    }
    let image_count = capabilities
        .min_image_count
        .max(2)
        .min(capabilities.max_image_count.unwrap_or(u32::MAX));
    let composite_alpha = capabilities
        .supported_composite_alpha
        .into_iter()
        .next()
        .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanUnsupportedFeature))?;
    let transform = capabilities.current_transform;

    let changed = info.image_extent != extent
        || info.min_image_count != image_count
        || info.composite_alpha != composite_alpha
        || info.pre_transform != transform;
    info.image_extent = extent;
    info.min_image_count = image_count;
    info.composite_alpha = composite_alpha;
    info.pre_transform = transform;
    Ok(changed)
}

fn surface_color_space_enabled(device: &Device, space: ColorSpace) -> bool {
    // Drivers can report extended spaces even when their swapchain extension is inactive.
    // Mirror ColorSpace's requirements in the pinned Vulkano swapchain validation.
    match space {
        ColorSpace::SrgbNonLinear => true,
        ColorSpace::DisplayNative => device.enabled_extensions().amd_display_native_hdr,
        _ => {
            device
                .instance()
                .enabled_extensions()
                .ext_swapchain_colorspace
        }
    }
}

fn surface_color_query_error(error: vk::Result) -> VMNLError {
    surface_error(
        &VulkanError::from(error).into(),
        VMNLErrorKind::VulkanSurfaceColorQueryFailed,
    )
}

fn surface_error(error: &Validated<VulkanError>, fallback: VMNLErrorKind) -> VMNLError {
    log::debug!("Vulkan surface/swapchain operation failed: {error}");
    VMNLError::new(match error {
        Validated::Error(VulkanError::SurfaceLost) => VMNLErrorKind::VulkanSurfaceLost,
        Validated::Error(VulkanError::DeviceLost) => VMNLErrorKind::VulkanDeviceLost,
        Validated::Error(VulkanError::OutOfHostMemory | VulkanError::OutOfDeviceMemory) => {
            VMNLErrorKind::VulkanOutOfMemory
        }
        _ => fallback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, Window};
    use vulkano::swapchain::{CompositeAlpha, CompositeAlphas, SurfaceTransform};

    #[test]
    #[ignore = "Requires Vulkan + GLFW display to obtain surface capabilities."]
    fn recreation_adapts_to_changed_surface_limits_and_preserves_color_pair() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let window = Window::builder().size(640, 480).build(&context)?;
        let swapchain = &window.inner.handle.swapchain;
        let mut capabilities = window
            .device()
            .physical_device()
            .surface_capabilities(swapchain.surface(), SurfaceInfo::default())
            .map_err(|error| surface_error(&error, VMNLErrorKind::VulkanSwapchainCreationFailed))?;
        let mut info = swapchain.create_info();
        let color_pair = (info.image_format, info.image_color_space);
        let present_mode = info.present_mode;
        info.min_image_count = 2;
        info.image_extent = [640, 480];
        capabilities.min_image_count = 3;
        capabilities.max_image_count = Some(4);
        capabilities.current_extent = Some([640, 480]);

        assert!(update_surface_options(
            &mut info,
            [640, 480],
            &capabilities
        )?);
        assert_eq!(info.min_image_count, 3);
        assert_eq!((info.image_format, info.image_color_space), color_pair);
        assert_eq!(info.present_mode, present_mode);
        assert!(!update_surface_options(
            &mut info,
            [640, 480],
            &capabilities
        )?);

        capabilities.min_image_count = 1;
        capabilities.max_image_count = Some(1);
        capabilities.current_extent = None;
        capabilities.min_image_extent = [320, 240];
        capabilities.max_image_extent = [800, 600];
        capabilities.current_transform = SurfaceTransform::Rotate90;
        capabilities.supported_composite_alpha = CompositeAlphas::PRE_MULTIPLIED;
        assert!(update_surface_options(
            &mut info,
            [200, 900],
            &capabilities
        )?);
        assert_eq!(info.image_extent, [320, 600]);
        assert_eq!(info.min_image_count, 1);
        assert_eq!(info.pre_transform, SurfaceTransform::Rotate90);
        assert_eq!(info.composite_alpha, CompositeAlpha::PreMultiplied);
        assert_eq!((info.image_format, info.image_color_space), color_pair);
        assert_eq!(info.present_mode, present_mode);

        capabilities.current_extent = Some([0, 480]);
        assert!(matches!(
            update_surface_options(&mut info, [640, 480], &capabilities),
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate)
        ));
        assert_eq!(info.image_extent, [320, 600]);
        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn framebuffer_failure_keeps_the_replacement_for_the_next_submit() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let mut window = Window::builder().size(640, 480).build(&context)?;
        window.render().submit()?;
        let before = Arc::clone(&window.inner.handle.swapchain);
        let pair = window.surface_color_info().pair();
        let result = window.inner.recreate_swapchain_with(|_, _| {
            Err(VMNLError::new(
                VMNLErrorKind::VulkanFramebufferCreationFailed,
            ))
        });
        assert!(
            matches!(result, Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanFramebufferCreationFailed))
        );
        let replacement = Arc::clone(&window.inner.handle.swapchain);
        assert!(!Arc::ptr_eq(&before, &replacement));
        assert!(matches!(
            window.inner.handle.swapchain_replacement,
            SwapchainReplacement::PendingFramebuffers(_)
        ));
        window.render().submit()?;
        assert!(Arc::ptr_eq(&replacement, &window.inner.handle.swapchain));
        assert!(matches!(
            window.inner.handle.swapchain_replacement,
            SwapchainReplacement::Ready
        ));
        assert_eq!(window.surface_color_info().pair(), pair);
        Ok(())
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn inactive_extensions_cannot_make_an_advertised_space_selectable() -> VMNLResult<()> {
        let _guard = crate::test_support::gpu_test_guard();
        let context = Context::new()?;
        let window = Window::builder().size(640, 480).build(&context)?;
        let device = window.device();
        if !device
            .instance()
            .enabled_extensions()
            .ext_swapchain_colorspace
        {
            let space = SurfaceColorSpace(ColorSpace::DisplayP3NonLinear);
            let pairs = window.surface_color_pairs()?;
            assert!(pairs.iter().all(|pair| pair.color_space() != space));
            let config = crate::SurfaceColorConfig::exact(
                window.surface_color_info().pair().format(),
                space,
            );
            let result = Window::builder()
                .size(640, 480)
                .surface_color(config.clone())
                .build(&context);
            assert!(
                matches!(result, Err(error) if matches!(error.kind(), VMNLErrorKind::SurfaceColorUnavailable { requested } if *requested == config))
            );
        }
        Ok(())
    }
}
