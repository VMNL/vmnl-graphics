// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Swapchain acquisition submodule for render frame setup.
//!
//! This module prepares frame futures and acquires the next swapchain image
//! before command submission.

use crate::{window::inner::VMNLWindow, VMNLError, VMNLErrorKind, VMNLResult};
use std::sync::Arc;
use vulkano::{
    swapchain::{self, Swapchain, SwapchainAcquireFuture},
    sync::GpuFuture,
    Validated, VulkanError,
};

#[cfg(test)]
thread_local! {
    pub(super) static ACQUIRE_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl VMNLWindow {
    /// Retires completed frame work before acquiring the next image.
    pub(super) fn begin_frame(&mut self) {
        #[cfg(test)]
        if self.handle.skip_frame_cleanup {
            return;
        }

        if let Some(previous_frame_end) = self.handle.previous_frame_end.as_mut() {
            previous_frame_end.cleanup_finished();
        }
    }

    /// Acquires the next available image from the swapchain for rendering.
    ///
    /// Returns a tuple `(image_index, suboptimal, acquire_future)` containing the image index,
    /// a boolean indicating if the swapchain is suboptimal, and the acquisition future.
    ///
    /// # Arguments
    /// - `swapchain`: Reference to the Vulkan swapchain.
    /// - `timeout`: Optional duration to wait for an image.
    pub(super) fn acquire_next_image_from_swapchain(
        swapchain: &Arc<Swapchain>,
        timeout: Option<std::time::Duration>,
    ) -> VMNLResult<(u32, bool, SwapchainAcquireFuture)> {
        #[cfg(test)]
        ACQUIRE_ATTEMPTS.set(ACQUIRE_ATTEMPTS.get() + 1);

        match swapchain::acquire_next_image(swapchain.clone(), timeout) {
            Ok(result) => Ok(result),
            Err(Validated::Error(VulkanError::OutOfDate)) => {
                Err(VMNLError::new(VMNLErrorKind::VulkanOutOfDate))
            }
            Err(error) => Err(VMNLError::new(VMNLErrorKind::InvalidState(format!(
                "{error:?}"
            )))),
        }
    }
}
