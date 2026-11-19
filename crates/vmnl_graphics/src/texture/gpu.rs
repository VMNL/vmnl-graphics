// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! GPU-backed texture resources and their explicit upload boundary.

use super::{
    AlphaMode, ColorEncoding, PixelSource, PngDecodeOptions, PreparedPixelData, SamplerConfig,
    SamplerLodLimit, TextureFormat, TextureUsage, TextureUsageRequest,
};
use crate::{
    vmnl_instance::VMNLInstance, Context, DeviceFeature, VMNLError, VMNLErrorKind, VMNLResult,
};
use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryAutoCommandBuffer,
    },
    device::Device,
    format::{Format, FormatFeatures},
    image::{
        sampler::{
            BorderColor, Filter as VulkanFilter, Sampler as VulkanSampler, SamplerAddressMode,
            SamplerCreateInfo, SamplerMipmapMode, LOD_CLAMP_NONE,
        },
        view::ImageView,
        Image, ImageCreateInfo, ImageLayout, ImageUsage,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    sync::future::{FenceSignalFuture, GpuFuture},
    sync::now,
};

const UPLOAD_POLL_INTERVAL: Duration = Duration::from_millis(1);

pub(crate) type TextureUploadDependency = Arc<FenceSignalFuture<Box<dyn GpuFuture + Send + Sync>>>;

#[derive(Clone)]
pub(crate) struct SampledTextureBinding {
    pub(crate) image_view: Arc<ImageView>,
    pub(crate) sampler: Arc<VulkanSampler>,
    pub(crate) device: Arc<Device>,
    pub(crate) upload_dependency: TextureUploadDependency,
    pub(crate) validation_error: Option<String>,
}

enum TextureSource<'a> {
    Pixels(PixelSource<'a>),
    Png(&'a [u8]),
    File(PathBuf),
}

/// Builder for a GPU texture sourced from RGBA8 pixels, encoded PNG data, or a PNG file.
///
/// Source decoding, image and staging allocation, and command-buffer preparation happen in
/// [`prepare`](Self::prepare). [`build`](Self::build) also submits the upload and returns without
/// waiting for it to complete. Borrowed input is no longer used after `prepare` returns.
pub struct Texture2DBuilder<'a> {
    context: Context,
    source: TextureSource<'a>,
    png_options: PngDecodeOptions,
    usage_override: Option<TextureUsageRequest>,
    retirement_policy: TextureRetirementPolicy,
}

impl<'a> Texture2DBuilder<'a> {
    /// Creates a builder from a borrowed RGBA8 pixel descriptor.
    #[must_use]
    pub fn from_pixels(context: &Context, source: PixelSource<'a>) -> Self {
        Self {
            context: context.clone(),
            source: TextureSource::Pixels(source),
            png_options: PngDecodeOptions::default(),
            usage_override: None,
            retirement_policy: TextureRetirementPolicy::Wait,
        }
    }

    /// Creates a builder from borrowed encoded PNG bytes.
    #[must_use]
    pub fn from_png(context: &Context, bytes: &'a [u8]) -> Self {
        Self {
            context: context.clone(),
            source: TextureSource::Png(bytes),
            png_options: PngDecodeOptions::default(),
            usage_override: None,
            retirement_policy: TextureRetirementPolicy::Wait,
        }
    }

    /// Creates a builder from an owned PNG path.
    #[must_use]
    pub fn from_file(context: &Context, path: impl Into<PathBuf>) -> Self {
        Self {
            context: context.clone(),
            source: TextureSource::File(path.into()),
            png_options: PngDecodeOptions::default(),
            usage_override: None,
            retirement_policy: TextureRetirementPolicy::Wait,
        }
    }

    /// Sets declarations used when decoding a PNG source.
    #[must_use]
    pub const fn with_png_options(mut self, options: PngDecodeOptions) -> Self {
        self.png_options = options;
        self
    }

    /// Replaces the image-usage request for this builder.
    ///
    /// `Automatic` selects sampled plus transfer-destination usage. An exact request is preserved;
    /// the initial upload fails if it omits transfer-destination usage.
    #[must_use]
    pub const fn with_usage(mut self, usage: TextureUsageRequest) -> Self {
        self.usage_override = Some(usage);
        self
    }

    /// Selects how the final texture owner retires its image resources.
    ///
    /// `Wait` is the default. `DeferToContext` requires explicit collection or context shutdown.
    #[must_use]
    pub const fn retirement_policy(mut self, policy: TextureRetirementPolicy) -> Self {
        self.retirement_policy = policy;
        self
    }

    /// Allocates the texture and records its initial upload without queue submission.
    ///
    /// Dropping the returned prepared value does not submit or wait. The upload bytes are copied
    /// into an owned staging buffer before this method returns.
    ///
    /// # Errors
    /// Returns a source, descriptor, image, staging-buffer, image-view, or command-buffer error.
    pub fn prepare(self) -> VMNLResult<PreparedTexture2D> {
        let (pixels, source_usage) = self.prepare_source()?;
        let usage_request = self.usage_override.unwrap_or(source_usage);
        let usage = match usage_request {
            TextureUsageRequest::Automatic => TextureUsage::INITIAL_UPLOAD,
            TextureUsageRequest::Exact(usage) => usage,
        };
        if !usage.contains(TextureUsage::TRANSFER_DESTINATION) {
            return Err(invalid_state(
                "initial texture upload requires transfer-destination usage",
            ));
        }

        let format = vulkan_format(pixels.format());
        let image_usage = vulkan_image_usage(usage);
        let image = Image::new(
            self.context.inner.memory_allocator.clone(),
            ImageCreateInfo {
                format,
                extent: [pixels.extent()[0], pixels.extent()[1], 1],
                usage: image_usage,
                initial_layout: ImageLayout::Undefined,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanImageCreationFailed))?;
        let view = ImageView::new_default(image.clone())
            .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanImageViewCreationFailed))?;
        let staging = Buffer::from_iter(
            self.context.inner.memory_allocator.clone(),
            BufferCreateInfo {
                usage: vulkano::buffer::BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            pixels.bytes().iter().copied(),
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanMemoryAllocationFailed))?;
        let command_buffer =
            record_initial_upload(&self.context.inner, image.clone(), staging.clone())?;

        Ok(PreparedTexture2D {
            context: self.context,
            image,
            view,
            staging,
            command_buffer,
            extent: pixels.extent(),
            format: pixels.format(),
            encoding: pixels.encoding(),
            alpha_mode: pixels.alpha_mode(),
            usage,
            retirement_policy: self.retirement_policy,
        })
    }

    /// Prepares and submits the initial upload, returning while it may still be pending.
    ///
    /// This method performs source I/O and/or PNG decoding, CPU and GPU allocations, command
    /// recording, and queue submission. Use [`prepare`](Self::prepare) and
    /// [`PreparedTexture2D::submit`] to choose the submission point explicitly.
    ///
    /// # Errors
    /// Returns the same errors as [`prepare`](Self::prepare) or an upload-submission error.
    pub fn build(self) -> VMNLResult<Texture2D> {
        self.prepare()?.submit()
    }

    fn prepare_source(&self) -> VMNLResult<(PreparedPixelData, TextureUsageRequest)> {
        let pixels = match &self.source {
            TextureSource::Pixels(source) => {
                let usage = source.usage();
                let pixels = source.prepare()?;
                return Ok((pixels, usage));
            }
            TextureSource::Png(bytes) => self.png_options.decode_png(bytes)?,
            TextureSource::File(path) => {
                let bytes = fs::read(path).map_err(|error| {
                    VMNLError::new(VMNLErrorKind::TextureSourceReadFailed(format!(
                        "{}: {error}",
                        path.display()
                    )))
                })?;
                self.png_options.decode_png(&bytes)?
            }
        };

        Ok((pixels, TextureUsageRequest::Automatic))
    }
}

/// Policy used when the final owner of a texture releases its GPU resources.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureRetirementPolicy {
    /// Wait for this texture's outstanding initial upload before releasing its resources.
    #[default]
    Wait,
    /// Retain pending resources in the creating context until explicit collection or shutdown.
    DeferToContext,
}

/// A GPU texture allocation with shared image storage and an initial-upload ticket.
///
/// Clones share one allocation. The final owner follows its selected retirement policy; query or
/// wait explicitly through [`initial_upload`](Self::initial_upload) when upload completion must be
/// visible in application code. Images use optimal tiling, one mip level, and device-preferred
/// memory with Vulkano's compatible fallback. The uploaded image remains in `General` layout for
/// the initial resource contract.
#[derive(Clone)]
pub struct Texture2D {
    storage: Rc<TextureStorage>,
}

struct TextureStorage {
    context: Context,
    image: Arc<Image>,
    default_view: Arc<ImageView>,
    upload: Rc<UploadState>,
    upload_dependency: TextureUploadDependency,
    extent: [u32; 2],
    format: TextureFormat,
    encoding: ColorEncoding,
    alpha_mode: AlphaMode,
    usage: TextureUsage,
    retirement_policy: TextureRetirementPolicy,
}

impl Drop for TextureStorage {
    fn drop(&mut self) {
        retire_texture_upload(self.retirement_policy, self.upload.clone(), |upload| {
            self.context
                .inner
                .texture_retirements
                .defer(RetiredTexture {
                    _image: self.image.clone(),
                    _view: self.default_view.clone(),
                    upload,
                });
        });
    }
}

fn retire_texture_upload(
    policy: TextureRetirementPolicy,
    upload: Rc<UploadState>,
    defer: impl FnOnce(Rc<UploadState>),
) {
    match policy {
        TextureRetirementPolicy::Wait => {
            let ticket = UploadTicket { state: upload };
            let _ = ticket.wait(None);
        }
        TextureRetirementPolicy::DeferToContext => defer(upload),
    }
}

impl Texture2D {
    /// Creates a builder from a borrowed RGBA8 pixel descriptor.
    #[must_use]
    pub fn from_pixels<'a>(context: &Context, source: PixelSource<'a>) -> Texture2DBuilder<'a> {
        Texture2DBuilder::from_pixels(context, source)
    }

    /// Creates a builder from borrowed encoded PNG bytes.
    #[must_use]
    pub fn from_png<'a>(context: &Context, bytes: &'a [u8]) -> Texture2DBuilder<'a> {
        Texture2DBuilder::from_png(context, bytes)
    }

    /// Creates a builder from an owned PNG path.
    #[must_use]
    pub fn from_file(context: &Context, path: impl Into<PathBuf>) -> Texture2DBuilder<'static> {
        Texture2DBuilder::from_file(context, path)
    }

    /// Returns the image extent in pixels.
    #[must_use]
    pub fn extent(&self) -> [u32; 2] {
        self.storage.extent
    }

    /// Returns the texture's RGBA8 format.
    #[must_use]
    pub fn format(&self) -> TextureFormat {
        self.storage.format
    }

    /// Returns the declared channel encoding; pixel values are not converted.
    #[must_use]
    pub fn encoding(&self) -> ColorEncoding {
        self.storage.encoding
    }

    /// Returns the declared alpha representation; pixel values are not converted.
    #[must_use]
    pub fn alpha_mode(&self) -> AlphaMode {
        self.storage.alpha_mode
    }

    /// Returns the exact image-usage set used by the Vulkan image.
    #[must_use]
    pub fn usage(&self) -> TextureUsage {
        self.storage.usage
    }

    /// Returns whether the exact image usage includes shader sampling.
    #[must_use]
    pub fn is_sampleable(&self) -> bool {
        self.storage.usage.contains(TextureUsage::SAMPLED)
    }

    /// Returns a handle to the default full-range view of the same image storage.
    #[must_use]
    pub fn view(&self) -> TextureView2D {
        TextureView2D {
            texture: self.clone(),
            inner: self.storage.default_view.clone(),
        }
    }

    /// Returns a reusable handle to the initial upload's completion state.
    #[must_use]
    pub fn initial_upload(&self) -> UploadTicket {
        UploadTicket {
            state: self.storage.upload.clone(),
        }
    }

    /// Returns whether this texture and another handle share the same image allocation.
    #[must_use]
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.storage, &other.storage)
    }

    pub(crate) fn belongs_to_context(&self, context: &Context) -> bool {
        Rc::ptr_eq(&self.storage.context.inner, &context.inner)
    }

    pub(crate) fn sampled_binding(&self, sampler: &Sampler) -> SampledTextureBinding {
        let view = self.view();
        let validation_error =
            view.validate_sampler(sampler)
                .err()
                .map(|error| match error.kind() {
                    VMNLErrorKind::InvalidState(message) => message.clone(),
                    _ => error.to_string(),
                });

        SampledTextureBinding {
            image_view: self.storage.default_view.clone(),
            sampler: sampler.inner.clone(),
            device: self.storage.context.inner.device.clone(),
            upload_dependency: self.storage.upload_dependency.clone(),
            validation_error,
        }
    }
}

/// A view that retains the texture storage it refers to.
#[derive(Clone)]
pub struct TextureView2D {
    texture: Texture2D,
    inner: Arc<ImageView>,
}

impl TextureView2D {
    /// Returns the texture that owns this view's image storage.
    #[must_use]
    pub const fn texture(&self) -> &Texture2D {
        &self.texture
    }

    /// Returns whether this view refers to the supplied texture allocation.
    #[must_use]
    pub fn shares_storage_with(&self, texture: &Texture2D) -> bool {
        self.texture.shares_storage_with(texture)
    }

    /// Returns the view's image extent in pixels.
    #[must_use]
    pub fn extent(&self) -> [u32; 2] {
        self.texture.extent()
    }

    /// Returns the view's format.
    #[must_use]
    pub fn format(&self) -> TextureFormat {
        self.texture.format()
    }

    /// Checks whether this view can be paired with a sampler on the same context.
    ///
    /// This validates sampled usage, context identity, linear-filter support, and the border
    /// color's numeric class. Shader instruction compatibility remains the consumer's
    /// responsibility.
    ///
    /// # Errors
    /// Returns `InvalidState` when the view and sampler are incompatible.
    pub fn validate_sampler(&self, sampler: &Sampler) -> VMNLResult<()> {
        if !self.texture.belongs_to_context(&sampler.context) {
            return Err(invalid_state(
                "texture view and sampler belong to different Context instances",
            ));
        }
        if !self.texture.is_sampleable() {
            return Err(invalid_state(
                "texture view image usage does not include shader sampling",
            ));
        }

        let config = sampler.config();
        let uses_linear_filter = [
            config.min_filter(),
            config.mag_filter(),
            config.mipmap_filter(),
        ]
        .contains(&super::Filter::Linear);
        if uses_linear_filter
            && !self
                .inner
                .format_features()
                .contains(FormatFeatures::SAMPLED_IMAGE_FILTER_LINEAR)
        {
            return Err(invalid_state(
                "texture view format does not support linear filtering",
            ));
        }

        let uses_border = [config.address_u(), config.address_v()]
            .into_iter()
            .any(|mode| mode == super::AddressMode::ClampToBorder);
        let integer_border = matches!(
            config.border_color(),
            super::SamplerBorderColor::IntTransparentBlack
                | super::SamplerBorderColor::IntOpaqueBlack
                | super::SamplerBorderColor::IntOpaqueWhite
        );
        if uses_border && integer_border {
            return Err(invalid_state(
                "RGBA8 normalized texture views require a floating-point sampler border color",
            ));
        }

        Ok(())
    }
}

/// A prepared texture that owns its staging data and recorded upload but has not been submitted.
///
/// Dropping this value releases its allocations without queue submission or waiting.
pub struct PreparedTexture2D {
    context: Context,
    image: Arc<Image>,
    view: Arc<ImageView>,
    staging: Subbuffer<[u8]>,
    command_buffer: Arc<PrimaryAutoCommandBuffer>,
    extent: [u32; 2],
    format: TextureFormat,
    encoding: ColorEncoding,
    alpha_mode: AlphaMode,
    usage: TextureUsage,
    retirement_policy: TextureRetirementPolicy,
}

impl PreparedTexture2D {
    /// Returns the prepared image extent in pixels.
    #[must_use]
    pub const fn extent(&self) -> [u32; 2] {
        self.extent
    }

    /// Submits the prepared upload and returns a texture with a reusable completion ticket.
    ///
    /// Queue submission is asynchronous. Dropping the final returned texture or ticket handle
    /// waits for this upload to complete.
    ///
    /// # Errors
    /// Returns `TextureUploadFailed` if command execution or queue submission fails.
    pub fn submit(self) -> VMNLResult<Texture2D> {
        let future = Arc::new(
            now(self.context.inner.device.clone())
                .then_execute(
                    self.context.inner.graphics_queue.clone(),
                    self.command_buffer,
                )
                .map_err(|error| upload_error(error.to_string()))?
                .boxed_send_sync()
                .then_signal_fence_and_flush()
                .map_err(|error| upload_error(error.to_string()))?,
        );
        let upload = Rc::new(UploadState {
            progress: RefCell::new(UploadProgress::Pending(PendingUpload::Vulkan {
                future: future.clone(),
                _staging: self.staging,
            })),
        });
        Ok(Texture2D {
            storage: Rc::new(TextureStorage {
                context: self.context,
                image: self.image,
                default_view: self.view,
                upload,
                upload_dependency: future,
                extent: self.extent,
                format: self.format,
                encoding: self.encoding,
                alpha_mode: self.alpha_mode,
                usage: self.usage,
                retirement_policy: self.retirement_policy,
            }),
        })
    }
}

/// Reusable initial-upload status handle.
#[derive(Clone)]
pub struct UploadTicket {
    state: Rc<UploadState>,
}

struct UploadState {
    progress: RefCell<UploadProgress>,
}

enum UploadProgress {
    Pending(PendingUpload),
    Complete,
    Failed(String),
}

enum PendingUpload {
    Vulkan {
        future: TextureUploadDependency,
        _staging: Subbuffer<[u8]>,
    },
    #[cfg(test)]
    Simulated {
        polls_until_signal: usize,
        poll_count: Rc<std::cell::Cell<usize>>,
        completion: Result<(), String>,
    },
}

impl PendingUpload {
    fn is_signaled(&mut self) -> Result<bool, String> {
        match self {
            Self::Vulkan { future, .. } => future.is_signaled().map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Simulated {
                polls_until_signal,
                poll_count,
                ..
            } => {
                poll_count.set(poll_count.get().saturating_add(1));
                if *polls_until_signal == 0 {
                    Ok(true)
                } else {
                    *polls_until_signal -= 1;
                    Ok(false)
                }
            }
        }
    }

    fn wait(&mut self) -> Result<(), String> {
        match self {
            Self::Vulkan { future, .. } => future.wait(None).map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Simulated { completion, .. } => completion.clone(),
        }
    }
}

impl UploadTicket {
    /// Returns whether the upload has completed; querying never blocks.
    ///
    /// # Errors
    /// Returns the persistent upload failure if the fence or completion operation failed.
    pub fn is_complete(&self) -> VMNLResult<bool> {
        self.poll()
    }

    /// Waits up to `timeout` for completion and returns a reusable completion result.
    ///
    /// Finite timeouts poll the fence and never call Vulkano's state-consuming timed wait.
    /// Repeated waits and cloned tickets observe the same result. `None` waits without a deadline.
    ///
    /// # Errors
    /// Returns the persistent upload failure if fence polling or finalization failed.
    pub fn wait(&self, timeout: Option<Duration>) -> VMNLResult<UploadWaitOutcome> {
        let started = Instant::now();
        loop {
            if self.poll()? {
                return Ok(UploadWaitOutcome::Complete);
            }

            let Some(timeout) = timeout else {
                thread::sleep(UPLOAD_POLL_INTERVAL);
                continue;
            };
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Ok(UploadWaitOutcome::TimedOut);
            }
            thread::sleep(UPLOAD_POLL_INTERVAL.min(timeout.saturating_sub(elapsed)));
        }
    }

    fn poll(&self) -> VMNLResult<bool> {
        let mut progress = self.state.progress.borrow_mut();
        let result = match &mut *progress {
            UploadProgress::Complete => return Ok(true),
            UploadProgress::Failed(message) => return Err(upload_error(message.clone())),
            UploadProgress::Pending(upload) => match upload.is_signaled() {
                Ok(false) => return Ok(false),
                Ok(true) => match upload.wait() {
                    Ok(()) => Ok(()),
                    Err(message) => Err(message),
                },
                Err(message) => {
                    // Vulkano's fence future must enter its cleaned state before being dropped.
                    let _ = upload.wait();
                    Err(message)
                }
            },
        };

        match result {
            Ok(()) => {
                *progress = UploadProgress::Complete;
                Ok(true)
            }
            Err(message) => {
                *progress = UploadProgress::Failed(message.clone());
                Err(upload_error(message))
            }
        }
    }
}

impl Drop for UploadState {
    fn drop(&mut self) {
        let progress = self.progress.get_mut();
        let completion = loop {
            match progress {
                UploadProgress::Pending(upload) => match upload.is_signaled() {
                    Ok(false) => thread::sleep(UPLOAD_POLL_INTERVAL),
                    Ok(true) => break upload.wait(),
                    Err(message) => {
                        let _ = upload.wait();
                        break Err(message);
                    }
                },
                UploadProgress::Complete | UploadProgress::Failed(_) => return,
            }
        };
        *progress = match completion {
            Ok(()) => UploadProgress::Complete,
            Err(message) => UploadProgress::Failed(message),
        };
    }
}

/// Result of a bounded or unbounded upload wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadWaitOutcome {
    /// The upload completed; this outcome remains observable through cloned tickets.
    Complete,
    /// The timeout elapsed; the upload remains pending and can be queried or waited again.
    TimedOut,
}

/// GPU sampler with its requested, unmodified configuration.
#[derive(Clone)]
pub struct Sampler {
    context: Context,
    inner: Arc<VulkanSampler>,
    config: SamplerConfig,
}

impl Sampler {
    /// Creates a sampler builder using the default linear configuration.
    #[must_use]
    pub fn builder(context: &Context) -> SamplerBuilder {
        SamplerBuilder {
            context: context.clone(),
            config: SamplerConfig::default(),
        }
    }

    /// Returns the exact configuration used to create this sampler.
    #[must_use]
    pub const fn config(&self) -> SamplerConfig {
        self.config
    }
}

struct RetiredTexture {
    _image: Arc<Image>,
    _view: Arc<ImageView>,
    upload: Rc<UploadState>,
}

pub(crate) struct TextureRetirementRegistry {
    resources: RefCell<Vec<RetiredTexture>>,
}

impl Default for TextureRetirementRegistry {
    fn default() -> Self {
        Self {
            resources: RefCell::new(Vec::new()),
        }
    }
}

impl std::fmt::Debug for TextureRetirementRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TextureRetirementRegistry")
            .field("pending_resources", &self.resources.borrow().len())
            .finish()
    }
}

impl TextureRetirementRegistry {
    fn defer(&self, texture: RetiredTexture) {
        self.resources.borrow_mut().push(texture);
    }

    pub(crate) fn collect(&self) -> VMNLResult<usize> {
        let mut resources = self.resources.borrow_mut();
        let mut collected = 0;
        let mut first_error = None;
        let mut index = 0;

        while index < resources.len() {
            let ticket = UploadTicket {
                state: resources[index].upload.clone(),
            };
            match ticket.poll() {
                Ok(true) => {
                    resources.remove(index);
                    collected += 1;
                }
                Ok(false) => index += 1,
                Err(error) => {
                    resources.remove(index);
                    collected += 1;
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(collected)
    }

    pub(crate) fn drain(&mut self) {
        for texture in self.resources.get_mut().drain(..) {
            let ticket = UploadTicket {
                state: texture.upload.clone(),
            };
            let _ = ticket.wait(None);
        }
    }
}

/// Builder for a GPU sampler.
pub struct SamplerBuilder {
    context: Context,
    config: SamplerConfig,
}

impl SamplerBuilder {
    /// Replaces the sampler configuration.
    #[must_use]
    pub const fn config(mut self, config: SamplerConfig) -> Self {
        self.config = config;
        self
    }

    /// Validates and creates the Vulkan sampler.
    ///
    /// Anisotropy and LOD bias are exact requests: they fail if required features or limits are
    /// unavailable. No sampler cache is used.
    ///
    /// # Errors
    /// Returns `InvalidState`, `DeviceFeatureNotEnabled`, or a Vulkan sampler creation error.
    pub fn build(self) -> VMNLResult<Sampler> {
        self.config.validate()?;
        let device = &self.context.inner.device;
        let physical = &self.context.inner.physical_device;
        if self.config.anisotropy() > 1.0 {
            if !device.enabled_features().sampler_anisotropy {
                return Err(VMNLError::new(VMNLErrorKind::DeviceFeatureNotEnabled {
                    feature: DeviceFeature::SamplerAnisotropy,
                }));
            }
            let limit = physical.properties().max_sampler_anisotropy;
            if self.config.anisotropy() > limit {
                return Err(invalid_state(format!(
                    "requested sampler anisotropy {} exceeds device limit {limit}",
                    self.config.anisotropy()
                )));
            }
        }
        if self.config.lod_bias() != 0.0
            && device.enabled_extensions().khr_portability_subset
            && !device.enabled_features().sampler_mip_lod_bias
        {
            return Err(VMNLError::new(VMNLErrorKind::DeviceFeatureNotEnabled {
                feature: DeviceFeature::SamplerMipLodBias,
            }));
        }
        if self.config.lod_bias().abs() > physical.properties().max_sampler_lod_bias {
            return Err(invalid_state(format!(
                "requested sampler LOD bias {} exceeds device limit {}",
                self.config.lod_bias(),
                physical.properties().max_sampler_lod_bias
            )));
        }

        let config = self.config;
        let anisotropy = (config.anisotropy() > 1.0).then_some(config.anisotropy());
        let max_lod = match config.max_lod() {
            SamplerLodLimit::Unbounded => LOD_CLAMP_NONE,
            SamplerLodLimit::Value(value) => value,
        };
        let inner = VulkanSampler::new(
            device.clone(),
            SamplerCreateInfo {
                mag_filter: vulkan_filter(config.mag_filter()),
                min_filter: vulkan_filter(config.min_filter()),
                mipmap_mode: match config.mipmap_filter() {
                    super::Filter::Nearest => SamplerMipmapMode::Nearest,
                    super::Filter::Linear => SamplerMipmapMode::Linear,
                },
                address_mode: [
                    vulkan_address_mode(config.address_u()),
                    vulkan_address_mode(config.address_v()),
                    vulkan_address_mode(config.address_v()),
                ],
                mip_lod_bias: config.lod_bias(),
                anisotropy,
                lod: config.min_lod()..=max_lod,
                border_color: vulkan_border_color(config.border_color()),
                unnormalized_coordinates: config.coordinate_mode()
                    == super::SamplerCoordinateMode::Texel,
                ..Default::default()
            },
        )
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanSamplerCreationFailed))?;

        Ok(Sampler {
            context: self.context,
            inner,
            config,
        })
    }
}

fn record_initial_upload(
    context: &VMNLInstance,
    image: Arc<Image>,
    staging: Subbuffer<[u8]>,
) -> VMNLResult<Arc<PrimaryAutoCommandBuffer>> {
    let mut command_buffer = AutoCommandBufferBuilder::primary(
        context.command_buffer_allocator.clone(),
        context.graphics_queue_family_index,
        CommandBufferUsage::OneTimeSubmit,
    )
    .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanCommandBufferCreationFailed))?;

    let mut copy = CopyBufferToImageInfo::buffer_image(staging, image);
    // General is valid for both the transfer and sampled use; retaining one layout also permits
    // exact usage requests that do not include sampled-image usage.
    copy.dst_image_layout = ImageLayout::General;
    command_buffer
        .copy_buffer_to_image(copy)
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanValidationFailed))?;
    command_buffer
        .build()
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanCommandBufferCreationFailed))
}

fn vulkan_format(format: TextureFormat) -> Format {
    match format {
        TextureFormat::Rgba8Unorm => Format::R8G8B8A8_UNORM,
        TextureFormat::Rgba8Srgb => Format::R8G8B8A8_SRGB,
    }
}

fn vulkan_image_usage(usage: TextureUsage) -> ImageUsage {
    let mut flags = ImageUsage::empty();
    if usage.contains(TextureUsage::SAMPLED) {
        flags |= ImageUsage::SAMPLED;
    }
    if usage.contains(TextureUsage::TRANSFER_DESTINATION) {
        flags |= ImageUsage::TRANSFER_DST;
    }
    if usage.contains(TextureUsage::TRANSFER_SOURCE) {
        flags |= ImageUsage::TRANSFER_SRC;
    }
    flags
}

fn vulkan_filter(filter: super::Filter) -> VulkanFilter {
    match filter {
        super::Filter::Nearest => VulkanFilter::Nearest,
        super::Filter::Linear => VulkanFilter::Linear,
    }
}

fn vulkan_address_mode(mode: super::AddressMode) -> SamplerAddressMode {
    match mode {
        super::AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
        super::AddressMode::Repeat => SamplerAddressMode::Repeat,
        super::AddressMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
        super::AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
    }
}

fn vulkan_border_color(color: super::SamplerBorderColor) -> BorderColor {
    match color {
        super::SamplerBorderColor::FloatTransparentBlack => BorderColor::FloatTransparentBlack,
        super::SamplerBorderColor::IntTransparentBlack => BorderColor::IntTransparentBlack,
        super::SamplerBorderColor::FloatOpaqueBlack => BorderColor::FloatOpaqueBlack,
        super::SamplerBorderColor::IntOpaqueBlack => BorderColor::IntOpaqueBlack,
        super::SamplerBorderColor::FloatOpaqueWhite => BorderColor::FloatOpaqueWhite,
        super::SamplerBorderColor::IntOpaqueWhite => BorderColor::IntOpaqueWhite,
    }
}

fn invalid_state(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::InvalidState(message.into()))
}

fn upload_error(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::TextureUploadFailed(message.into()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::*;
    use crate::{
        raw,
        texture::{AddressMode, PixelLayout, PixelOrigin, SamplerBorderColor},
        Context, Window,
    };
    use vulkano::{
        buffer::BufferUsage, command_buffer::CopyImageToBufferInfo,
        memory::allocator::MemoryTypeFilter,
    };

    #[test]
    fn upload_future_can_be_shared_with_raw_resources() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<TextureUploadDependency>();
    }

    fn simulated_upload(
        polls_until_signal: usize,
        completion: Result<(), String>,
    ) -> (Rc<UploadState>, Rc<std::cell::Cell<usize>>) {
        let poll_count = Rc::new(std::cell::Cell::new(0));
        let state = Rc::new(UploadState {
            progress: RefCell::new(UploadProgress::Pending(PendingUpload::Simulated {
                polls_until_signal,
                poll_count: poll_count.clone(),
                completion,
            })),
        });
        (state, poll_count)
    }

    #[test]
    fn failed_upload_state_remains_observable_through_ticket_clones() {
        let (state, poll_count) = simulated_upload(0, Err("simulated upload failure".into()));
        let ticket = UploadTicket { state };
        let clone = ticket.clone();

        for error in [
            ticket
                .is_complete()
                .expect_err("failed upload must remain an error"),
            clone
                .is_complete()
                .expect_err("cloned failure must persist"),
            ticket
                .wait(Some(Duration::ZERO))
                .expect_err("waiting on a failed upload must remain an error"),
        ] {
            assert!(matches!(
                error.kind(),
                VMNLErrorKind::TextureUploadFailed(message)
                    if message == "simulated upload failure"
            ));
        }
        assert_eq!(poll_count.get(), 1);
    }

    #[test]
    fn wait_retirement_polls_pending_upload_until_completion() {
        let (state, poll_count) = simulated_upload(2, Ok(()));
        let deferred = Rc::new(std::cell::Cell::new(false));
        let deferred_callback = deferred.clone();

        retire_texture_upload(TextureRetirementPolicy::Wait, state, move |_| {
            deferred_callback.set(true);
        });

        assert_eq!(poll_count.get(), 3);
        assert!(!deferred.get());
    }

    fn gpu_test_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::test_support::gpu_test_guard()
    }

    #[repr(C)]
    #[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
    struct RawTextureVertex {
        #[format(R32G32_SFLOAT)]
        position: [f32; 2],
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn raw_sampled_resources_reject_image_without_sampled_usage() -> VMNLResult<()> {
        let _guard = gpu_test_guard();
        let context = Context::new()?;
        let window = Window::builder().build(&context)?;
        let pipeline = raw::Pipeline::<RawTextureVertex>::builder()
            .vertex_shader(raw::ShaderSource::Src(
                "#version 450\nlayout(location = 0) in vec2 position;\nvoid main() { gl_Position = vec4(position, 0.0, 1.0); }"
                    .into(),
            ))
            .fragment_shader(raw::ShaderSource::Src(
                "#version 450\nlayout(set = 0, binding = 0) uniform sampler2D image;\nlayout(location = 0) out vec4 color;\nvoid main() { color = texture(image, vec2(0.5)); }"
                    .into(),
            ))
            .build(&window)?;
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()?;
        let dependency_texture = Texture2D::from_pixels(
            &context,
            PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &[255, 255, 255, 255])?,
        )
        .build()?;

        // Exercise the raw resource guard with a view-compatible GPU image whose
        // image usage deliberately omits SAMPLED. The public texture builder cannot
        // currently create this combination because it always creates a default view.
        let image = Image::new(
            context.inner.memory_allocator.clone(),
            ImageCreateInfo {
                format: Format::R8G8B8A8_UNORM,
                extent: [1, 1, 1],
                usage: ImageUsage::COLOR_ATTACHMENT,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .map_err(|error| upload_error(error.to_string()))?;
        let default_view = ImageView::new_default(image.clone())
            .map_err(|error| upload_error(error.to_string()))?;
        let texture = Texture2D {
            storage: Rc::new(TextureStorage {
                context: context.clone(),
                image,
                default_view,
                upload: Rc::new(UploadState {
                    progress: RefCell::new(UploadProgress::Complete),
                }),
                upload_dependency: dependency_texture.storage.upload_dependency.clone(),
                extent: [1, 1],
                format: TextureFormat::Rgba8Unorm,
                encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Opaque,
                usage: TextureUsage::TRANSFER_DESTINATION,
                retirement_policy: TextureRetirementPolicy::Wait,
            }),
        };
        let Err(error) = raw::Resources::builder(&pipeline)
            .sampled_texture(0, 0, &texture, &sampler)
            .build(&context)
        else {
            panic!("a non-sampled image must be rejected");
        };
        match error.kind() {
            VMNLErrorKind::InvalidState(message)
                if message
                    == "raw resources set 0 binding 0: texture view image usage does not include shader sampling" => {}
            kind => panic!("unexpected non-sampled texture rejection: {kind:?}"),
        }

        Ok(())
    }

    fn readback_rgba8(texture: &Texture2D) -> Vec<u8> {
        let context = &texture.storage.context;
        let byte_count = usize::try_from(texture.extent()[0])
            .unwrap()
            .checked_mul(usize::try_from(texture.extent()[1]).unwrap())
            .and_then(|pixels| pixels.checked_mul(4))
            .unwrap();
        let buffer = Buffer::new_slice::<u8>(
            context.inner.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            u64::try_from(byte_count).unwrap(),
        )
        .expect("readback buffer should allocate");
        let mut command_buffer = AutoCommandBufferBuilder::primary(
            context.inner.command_buffer_allocator.clone(),
            context.inner.graphics_queue_family_index,
            CommandBufferUsage::OneTimeSubmit,
        )
        .expect("readback command buffer should allocate");
        let mut copy =
            CopyImageToBufferInfo::image_buffer(texture.storage.image.clone(), buffer.clone());
        copy.src_image_layout = ImageLayout::General;
        command_buffer
            .copy_image_to_buffer(copy)
            .expect("image readback should record");
        let command_buffer = command_buffer
            .build()
            .expect("readback command buffer should build");
        now(context.inner.device.clone())
            .then_execute(context.inner.graphics_queue.clone(), command_buffer)
            .expect("readback should submit")
            .then_signal_fence_and_flush()
            .expect("readback fence should signal")
            .wait(None)
            .expect("readback should complete");
        let pixels = buffer
            .read()
            .expect("readback buffer should be readable")
            .to_vec();
        pixels
    }

    fn padded_pixels(origin: PixelOrigin) -> Vec<u8> {
        let mut bytes = vec![0xee; 28];
        let (first_row, second_row) = match origin {
            PixelOrigin::TopLeft => (
                [255, 0, 0, 255, 0, 255, 0, 255],
                [0, 0, 255, 255, 255, 255, 255, 255],
            ),
            PixelOrigin::BottomLeft => (
                [0, 0, 255, 255, 255, 255, 255, 255],
                [255, 0, 0, 255, 0, 255, 0, 255],
            ),
        };
        bytes[2..4].copy_from_slice(&[0xa1, 0xa2]);
        bytes[6..14].copy_from_slice(&first_row);
        bytes[14..18].copy_from_slice(&[0xc1, 0xc2, 0xc3, 0xc4]);
        bytes[18..26].copy_from_slice(&second_row);
        bytes[26..28].copy_from_slice(&[0xd1, 0xd2]);
        bytes
    }

    fn build_readable_texture(context: &Context, origin: PixelOrigin) -> Texture2D {
        let bytes = padded_pixels(origin);
        let source = PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &bytes)
            .expect("pixel source should be valid")
            .with_byte_range(2..26)
            .with_layout(PixelLayout::new(4, 12, origin))
            .with_usage(TextureUsageRequest::Exact(
                TextureUsage::INITIAL_UPLOAD.union(TextureUsage::TRANSFER_SOURCE),
            ));
        Texture2D::from_pixels(context, source)
            .build()
            .expect("texture should upload")
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn upload_readback_preserves_texels_padding_and_top_left_origin() {
        let _guard = gpu_test_guard();
        let context = Context::new().expect("context should initialize");
        let expected = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];

        for origin in [PixelOrigin::TopLeft, PixelOrigin::BottomLeft] {
            let texture = build_readable_texture(&context, origin);
            texture
                .initial_upload()
                .wait(None)
                .expect("upload should complete");
            assert_eq!(readback_rgba8(&texture), expected);
        }
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn texture_upload_tickets_are_reusable_across_same_context_windows() {
        let _guard = gpu_test_guard();
        let context = Context::new().expect("context should initialize");
        let first_context = context.clone();
        let second_context = context.clone();
        let pixels = [255, 255, 255, 255];
        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)
            .expect("pixel source should be valid");
        let texture = Texture2D::from_pixels(&context, source)
            .build()
            .expect("texture should upload");
        let first_upload_observer = texture.initial_upload();
        let second_upload_observer = first_upload_observer.clone();
        let view = texture.view();
        let sampler = Sampler::builder(&context)
            .config(SamplerConfig::nearest())
            .build()
            .expect("nearest sampler should be supported");
        view.validate_sampler(&sampler)
            .expect("same-context view and sampler should be compatible");
        let integer_border_sampler = Sampler::builder(&context)
            .config(
                SamplerConfig::nearest()
                    .with_address_u(AddressMode::ClampToBorder)
                    .with_border_color(SamplerBorderColor::IntOpaqueBlack),
            )
            .build()
            .expect("integer border sampler should create independently");
        assert!(matches!(
            view.validate_sampler(&integer_border_sampler),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message)
                if message.contains("floating-point sampler border color"))
        ));

        // Textured renderer binding is not part of this resource step. These frames prove the
        // texture handles coexist with two windows created from clones of their owning Context.
        let mut first_window = Window::builder()
            .title("texture resource sharing one")
            .build(&first_context)
            .expect("first window should initialize");
        let mut second_window = Window::builder()
            .title("texture resource sharing two")
            .build(&second_context)
            .expect("second window should initialize");
        first_window
            .render()
            .submit()
            .expect("first context window should render");
        assert_eq!(
            first_upload_observer
                .wait(None)
                .expect("first upload observer should complete"),
            UploadWaitOutcome::Complete
        );
        second_window
            .render()
            .submit()
            .expect("second context window should render");
        assert_eq!(
            second_upload_observer
                .wait(None)
                .expect("second upload observer should see the same completion"),
            UploadWaitOutcome::Complete
        );

        let separate_context = Context::new().expect("separate context should initialize");
        assert_eq!(context.device_name(), separate_context.device_name());
        let separate_sampler = Sampler::builder(&separate_context)
            .config(SamplerConfig::nearest())
            .build()
            .expect("separate-context sampler should initialize");
        assert!(matches!(
            view.validate_sampler(&separate_sampler),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message)
                if message.contains("different Context"))
        ));
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn deferred_texture_retirement_waits_for_final_view_and_is_collectable() {
        let _guard = gpu_test_guard();
        let context = Context::new().expect("context should initialize");
        let pixels = [1, 2, 3, 255];
        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)
            .expect("pixel source should be valid");
        let texture = Texture2D::from_pixels(&context, source)
            .retirement_policy(TextureRetirementPolicy::DeferToContext)
            .build()
            .expect("texture should upload");
        let ticket = texture.initial_upload();
        let view: TextureView2D = texture.view();
        let clone = texture.clone();

        drop(texture);
        drop(clone);
        assert_eq!(context.collect_texture_retirements().unwrap(), 0);
        drop(view);

        ticket
            .wait(None)
            .expect("upload should complete before collection");
        assert_eq!(context.collect_texture_retirements().unwrap(), 1);
        assert!(ticket.is_complete().unwrap());
        assert_eq!(context.collect_texture_retirements().unwrap(), 0);
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn deferred_texture_retirement_drains_when_context_is_dropped() {
        let _guard = gpu_test_guard();
        let context = Context::new().expect("context should initialize");
        let pixels = [4, 5, 6, 255];
        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)
            .expect("pixel source should be valid");
        let texture = Texture2D::from_pixels(&context, source)
            .retirement_policy(TextureRetirementPolicy::DeferToContext)
            .build()
            .expect("texture should upload");

        drop(texture);
        drop(context);
    }

    #[test]
    #[ignore = "Requires Vulkan + GLFW display."]
    fn wait_texture_retirement_releases_after_the_final_view_drops() {
        let _guard = gpu_test_guard();
        let context = Context::new().expect("context should initialize");
        let pixels = [7, 8, 9, 255];
        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &pixels)
            .expect("pixel source should be valid");
        let texture = Texture2D::from_pixels(&context, source)
            .retirement_policy(TextureRetirementPolicy::Wait)
            .build()
            .expect("texture should upload");
        let upload = texture.initial_upload();
        let view = texture.view();

        upload
            .wait(None)
            .expect("upload should complete before retirement");
        drop(texture);
        assert_eq!(context.collect_texture_retirements().unwrap(), 0);
        drop(view);
        assert_eq!(context.collect_texture_retirements().unwrap(), 0);
    }
}
