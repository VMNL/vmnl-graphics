// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Public texture-resource upload checks requiring Vulkan and GLFW.

use std::time::Duration;
use vmnl_gpu_tests::gpu_test_guard;
use vmnl_graphics::{
    texture::{
        PixelSource, Sampler, Texture2D, TextureFormat, TextureUsage, TextureUsageRequest,
        UploadWaitOutcome,
    },
    Context, VMNLErrorKind,
};

#[test]
#[ignore = "Requires Vulkan + GLFW display."]
fn upload_ticket_is_reusable_and_default_view_shares_storage() {
    let _guard = gpu_test_guard();
    let context = Context::new().expect("context should initialize");
    let pixels = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let source = PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &pixels)
        .expect("pixel descriptor should be valid");
    let prepared = Texture2D::from_pixels(&context, source)
        .prepare()
        .expect("texture preparation should succeed");
    let texture = prepared.submit().expect("upload submission should succeed");
    let clone = texture.clone();
    let view = texture.view();

    assert!(texture.shares_storage_with(&clone));
    assert!(view.shares_storage_with(&texture));
    assert_eq!(view.extent(), [2, 2]);
    assert!(texture.usage().contains(TextureUsage::SAMPLED));
    assert!(texture.usage().contains(TextureUsage::TRANSFER_DESTINATION));

    let ticket = texture.initial_upload();
    assert!(matches!(
        ticket.wait(Some(Duration::ZERO)),
        Ok(UploadWaitOutcome::Complete | UploadWaitOutcome::TimedOut)
    ));
    assert_eq!(
        ticket.wait(None).expect("upload should complete"),
        UploadWaitOutcome::Complete
    );
    assert!(ticket
        .is_complete()
        .expect("completed ticket should remain reusable"));
    assert_eq!(
        clone
            .initial_upload()
            .wait(None)
            .expect("cloned ticket should complete"),
        UploadWaitOutcome::Complete
    );
}

#[test]
#[ignore = "Requires Vulkan + GLFW display."]
fn exact_usage_without_transfer_destination_is_rejected_before_allocation() {
    let _guard = gpu_test_guard();
    let context = Context::new().expect("context should initialize");
    let pixels = [0, 0, 0, 255];
    let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)
        .expect("pixel descriptor should be valid")
        .with_usage(TextureUsageRequest::Exact(TextureUsage::SAMPLED));

    let error = Texture2D::from_pixels(&context, source)
        .prepare()
        .err()
        .expect("the exact usage request cannot be silently expanded");
    assert!(matches!(error.kind(), VMNLErrorKind::InvalidState(_)));
}

#[test]
#[ignore = "Requires Vulkan + GLFW display."]
fn default_sampler_preserves_its_requested_configuration() {
    let _guard = gpu_test_guard();
    let context = Context::new().expect("context should initialize");
    let sampler = Sampler::builder(&context)
        .build()
        .expect("default sampler should be supported");

    assert_eq!(sampler.config().anisotropy(), 1.0);
}
