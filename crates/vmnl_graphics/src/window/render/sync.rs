// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Render synchronization submodule for GPU futures and presentation.
//!
//! This module chains image acquisition, command execution, presentation,
//! and fence signaling for a rendered frame.

use crate::window::runtime::FrameCompletion;
use crate::{VMNLError, VMNLErrorKind, VMNLResult, window::inner::VMNLWindow};
use std::sync::Arc;
use vulkano::{
    Validated, VulkanError,
    command_buffer::PrimaryAutoCommandBuffer,
    device::Queue,
    swapchain::{Swapchain, SwapchainAcquireFuture, SwapchainPresentInfo},
    sync::GpuFuture,
};

pub(super) enum FrameSyncResult<T> {
    Submitted(T),
    // `None` means the failed Vulkano future already retired submitted work.
    SwapchainOutOfDate(Option<T>),
}

pub(super) fn finalize_frame_sync<T>(
    frame_sync: VMNLResult<FrameSyncResult<T>>,
    swapchain_recreation_requested: &mut bool,
    mut install_completion: impl FnMut(Option<T>),
) -> VMNLResult<()> {
    match frame_sync {
        Err(error) => Err(error),
        Ok(FrameSyncResult::Submitted(completion)) => {
            install_completion(Some(completion));
            Ok(())
        }
        Ok(FrameSyncResult::SwapchainOutOfDate(completion)) => {
            log::warn!(
                "{}",
                VMNLError::new(VMNLErrorKind::VulkanOutOfDate).report()
            );
            *swapchain_recreation_requested = true;
            install_completion(completion);
            Err(VMNLError::new(VMNLErrorKind::VulkanOutOfDate))
        }
    }
}

impl VMNLWindow {
    /// Synchronizes rendering with presentation by chaining GPU futures for the current frame.
    ///
    /// # Arguments
    /// - `previous_frame_end`: Mutable reference to the previous frame future.
    /// - `acquire_future`: Future for image acquisition.
    /// - `command_buffer`: Command buffer for current frame.
    /// - `image_index`: Index of the acquired image.
    /// - `graphics_queue`: Queue to execute command buffer.
    /// - `swapchain`: Swapchain used for presentation.
    ///
    pub(super) fn frame_sync(
        previous_frame_end: &mut Option<Box<dyn GpuFuture + Send + Sync>>,
        acquire_future: SwapchainAcquireFuture,
        command_buffer: Arc<PrimaryAutoCommandBuffer>,
        image_index: u32,
        graphics_queue: Arc<Queue>,
        swapchain: Arc<Swapchain>,
    ) -> VMNLResult<FrameSyncResult<FrameCompletion>> {
        let previous = previous_frame_end
            .take()
            .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanUnknownError))?;
        let after_exec = previous
            .join(acquire_future)
            .then_execute(graphics_queue.clone(), command_buffer)
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
        let after_present = after_exec.then_swapchain_present(
            graphics_queue,
            SwapchainPresentInfo::swapchain_image_index(swapchain, image_index),
        );
        match after_present
            .boxed_send_sync()
            .then_signal_fence_and_flush()
        {
            Ok(flushed) => Ok(FrameSyncResult::Submitted(Arc::new(flushed))),
            Err(Validated::Error(VulkanError::OutOfDate)) => {
                // In Vulkano 0.35.2, dropping the failed fence future waits for the present
                // queue and retires its submitted resources before this result is returned.
                Ok(FrameSyncResult::SwapchainOutOfDate(None))
            }
            Err(error) => {
                log::error!(
                    "{}: {error:?}",
                    VMNLError::new(VMNLErrorKind::VulkanUnknownError).report()
                );
                Err(VMNLError::new(VMNLErrorKind::VulkanValidationFailed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::{FrameSyncResult, finalize_frame_sync};
    use crate::{VMNLError, VMNLErrorKind, VMNLResult};
    use std::sync::Arc;

    #[test]
    fn submitted_completion_is_retained_for_global_and_depth_ownership() {
        let completion = Arc::new(());
        let mut previous_frame_end = None;
        let mut depth_frame_completion = None;
        let mut recreation_requested = false;

        let result = finalize_frame_sync(
            Ok(FrameSyncResult::Submitted(completion.clone())),
            &mut recreation_requested,
            |completion| {
                let Some(completion) = completion else {
                    panic!("a successful submission must have a completion");
                };
                previous_frame_end = Some(completion.clone());
                depth_frame_completion = Some(completion);
            },
        );

        assert!(result.is_ok());
        assert!(!recreation_requested);
        assert!(matches!(
            previous_frame_end.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &completion)
        ));
        assert!(matches!(
            depth_frame_completion.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &completion)
        ));
    }

    #[test]
    fn out_of_date_returns_error_after_requesting_recreation_and_retaining_completion() {
        let completion = Arc::new(());
        let mut previous_frame_end = None;
        let mut depth_frame_completion = None;
        let mut recreation_requested = false;

        let result = finalize_frame_sync(
            Ok(FrameSyncResult::SwapchainOutOfDate(Some(
                completion.clone(),
            ))),
            &mut recreation_requested,
            |completion| {
                let Some(completion) = completion else {
                    panic!("an in-flight completion must be retained");
                };
                previous_frame_end = Some(completion.clone());
                depth_frame_completion = Some(completion);
            },
        );

        assert!(recreation_requested);
        assert!(matches!(
            previous_frame_end.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &completion)
        ));
        assert!(matches!(
            depth_frame_completion.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &completion)
        ));
        assert!(matches!(
            result,
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate)
        ));
    }

    #[test]
    fn existing_recreation_request_survives_out_of_date() {
        let mut recreation_requested = true;
        let mut installed_ready_future = false;
        let frame_sync: VMNLResult<FrameSyncResult<Arc<()>>> =
            Ok(FrameSyncResult::SwapchainOutOfDate(None));

        let result = finalize_frame_sync(frame_sync, &mut recreation_requested, |completion| {
            installed_ready_future = completion.is_none()
        });

        assert!(recreation_requested);
        assert!(installed_ready_future);
        assert!(matches!(
            result,
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate)
        ));
    }

    #[test]
    fn other_sync_errors_keep_their_kind_and_do_not_request_recreation() {
        let mut recreation_requested = false;
        let mut retained_completion = false;
        let frame_sync: VMNLResult<FrameSyncResult<Arc<()>>> =
            Err(VMNLError::new(VMNLErrorKind::VulkanDeviceLost));

        let result = finalize_frame_sync(frame_sync, &mut recreation_requested, |_| {
            retained_completion = true
        });

        assert!(!recreation_requested);
        assert!(!retained_completion);
        assert!(matches!(
            result,
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanDeviceLost)
        ));
    }

    #[test]
    fn a_valid_retry_succeeds_after_out_of_date_recreation() {
        let failed_completion = Arc::new(());
        let retried_completion = Arc::new(());
        let mut previous_frame_end = None;
        let mut depth_frame_completion = None;
        let mut recreation_requested = false;

        let first_result = finalize_frame_sync(
            Ok(FrameSyncResult::SwapchainOutOfDate(Some(
                failed_completion.clone(),
            ))),
            &mut recreation_requested,
            |completion| {
                let Some(completion) = completion else {
                    panic!("an in-flight completion must be retained");
                };
                previous_frame_end = Some(completion.clone());
                depth_frame_completion = Some(completion);
            },
        );
        let first_result_was_out_of_date = matches!(
            first_result,
            Err(error) if matches!(error.kind(), VMNLErrorKind::VulkanOutOfDate)
        );
        assert!(recreation_requested);
        assert!(matches!(
            previous_frame_end.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &failed_completion)
        ));

        recreation_requested = false;
        let retry_result = finalize_frame_sync(
            Ok(FrameSyncResult::Submitted(retried_completion.clone())),
            &mut recreation_requested,
            |completion| {
                let Some(completion) = completion else {
                    panic!("a successful retry must have a completion");
                };
                previous_frame_end = Some(completion.clone());
                depth_frame_completion = Some(completion);
            },
        );

        assert!(retry_result.is_ok());
        assert!(!recreation_requested);
        assert!(matches!(
            previous_frame_end.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &retried_completion)
        ));
        assert!(matches!(
            depth_frame_completion.as_ref(),
            Some(retained) if Arc::ptr_eq(retained, &retried_completion)
        ));
        assert!(first_result_was_out_of_date);
    }
}
