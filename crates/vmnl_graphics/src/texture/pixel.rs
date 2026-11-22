// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Pixel source descriptors and checked CPU-side normalization.

use std::ops::Range;

use crate::{VMNLError, VMNLErrorKind, VMNLResult};

/// An RGBA8 image representation accepted by the texture source API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFormat {
    /// Four 8-bit channels sampled as linear values or data.
    Rgba8Unorm,
    /// Four 8-bit channels sampled with native sRGB decoding for RGB.
    Rgba8Srgb,
}

impl TextureFormat {
    pub(crate) const fn bytes_per_pixel() -> usize {
        4
    }

    const fn default_encoding(self) -> ColorEncoding {
        match self {
            Self::Rgba8Unorm => ColorEncoding::Linear,
            Self::Rgba8Srgb => ColorEncoding::Srgb,
        }
    }
}

/// Declared interpretation of the supplied RGB channel values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorEncoding {
    /// RGB values represent linear-light or non-color data.
    Linear,
    /// RGB values use the sRGB transfer function.
    Srgb,
}

/// Declared alpha representation of supplied pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    /// RGB channels are independent of alpha; PNG alpha uses this representation.
    Straight,
    /// RGB channels have already been multiplied by alpha.
    Premultiplied,
    /// All pixels are declared fully opaque.
    Opaque,
}

/// Vertical origin of the first row in the supplied pixel source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PixelOrigin {
    /// The first row is the logical top row.
    #[default]
    TopLeft,
    /// The first row is the logical bottom row.
    BottomLeft,
}

/// Byte layout for rows in an uncompressed pixel source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelLayout {
    offset: usize,
    row_stride: usize,
    origin: PixelOrigin,
}

impl PixelLayout {
    /// Creates a byte layout. `offset` is relative to the source byte subrange.
    ///
    /// # Arguments
    ///
    /// - `offset` supplies the byte offset from the start of the declared source range to the first
    ///   stored row.
    /// - `row_stride` supplies the byte distance between consecutive stored rows, including any
    ///   padding.
    /// - `origin` identifies which image row is stored first.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn new(offset: usize, row_stride: usize, origin: PixelOrigin) -> Self {
        Self {
            offset,
            row_stride,
            origin,
        }
    }

    /// Returns the byte offset from the beginning of the declared source subrange.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the distance in bytes between the starts of adjacent input rows.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn row_stride(self) -> usize {
        self.row_stride
    }

    /// Returns the logical origin of the first input row.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn origin(self) -> PixelOrigin {
        self.origin
    }
}

/// A non-empty, half-open rectangle in top-left-origin pixel coordinates.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::texture::PixelRect;
///
/// let region = PixelRect::new(8, 16, 32, 32)?;
/// region.validate_within(64, 64)?;
/// assert_eq!([region.x(), region.y(), region.width(), region.height()], [8, 16, 32, 32]);
/// # Ok::<(), vmnl_graphics::VMNLError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl PixelRect {
    /// Creates a rectangle, rejecting empty extents and overflowing ends.
    ///
    /// # Arguments
    ///
    /// - `x` supplies the horizontal start in texels from the left edge.
    /// - `y` supplies the vertical start in texels from the top edge.
    /// - `width` supplies the nonzero horizontal extent in texels.
    /// - `height` supplies the nonzero vertical extent in texels.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when either dimension is zero or either end overflows.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> VMNLResult<Self> {
        if width == 0 || height == 0 {
            return Err(invalid_state("pixel rectangle dimensions must be nonzero"));
        }
        if x.checked_add(width).is_none() || y.checked_add(height).is_none() {
            return Err(invalid_state(
                "pixel rectangle end overflows its coordinate range",
            ));
        }

        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    /// Returns the left edge in pixels.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn x(self) -> u32 {
        self.x
    }

    /// Returns the top edge in pixels.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn y(self) -> u32 {
        self.y
    }

    /// Returns the rectangle width in pixels.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the rectangle height in pixels.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Checks that the half-open rectangle is within an image extent.
    ///
    /// # Arguments
    ///
    /// - `image_width` supplies the source image width in texels.
    /// - `image_height` supplies the source image height in texels.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when the rectangle extends outside the image.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    pub fn validate_within(self, image_width: u32, image_height: u32) -> VMNLResult<()> {
        if self.x + self.width > image_width || self.y + self.height > image_height {
            return Err(invalid_state("pixel rectangle is outside the image extent"));
        }

        Ok(())
    }
}

/// Requested image usages for an eventual texture allocation.
///
/// This is a closed bit set. Image support and the selected effective usage are
/// checked against a `Context` when GPU resource creation is introduced.
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::texture::TextureUsage;
///
/// let usage = TextureUsage::SAMPLED.union(TextureUsage::TRANSFER_DESTINATION);
/// assert!(usage.contains(TextureUsage::INITIAL_UPLOAD));
/// assert!(!usage.is_empty());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureUsage(u8);

impl TextureUsage {
    /// Usage for sampling an image from a shader.
    pub const SAMPLED: Self = Self(1 << 0);
    /// Usage for the initial buffer-to-image upload and later updates.
    pub const TRANSFER_DESTINATION: Self = Self(1 << 1);
    /// Usage required for explicit image-to-buffer or image-to-image transfers.
    pub const TRANSFER_SOURCE: Self = Self(1 << 2);
    /// Default usages for an automatically configured initial upload.
    pub const INITIAL_UPLOAD: Self = Self(Self::SAMPLED.0 | Self::TRANSFER_DESTINATION.0);

    /// Returns the union of two usage sets.
    ///
    /// # Arguments
    ///
    /// - `other` supplies the additional usage flags to include.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether every bit in `other` is present in this usage set.
    ///
    /// # Arguments
    ///
    /// - `other` supplies the complete set of flags to test.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether this set requests no image usages.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Whether image usages are selected automatically or requested exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureUsageRequest {
    /// Request the documented initial-upload usages when GPU allocation is built.
    #[default]
    Automatic,
    /// Request exactly this usage set; unsupported combinations must fail unchanged.
    Exact(TextureUsage),
}

/// A borrowed RGBA8 pixel source with explicit format, usage, range, and row layout.
///
/// New sources default to tightly packed top-left rows, the encoding associated
/// with the requested format, straight alpha, and automatic image usage. The
/// implicit byte range is the complete supplied slice and therefore requires
/// its length to match the calculated required end. Use [`with_byte_range`]
/// when a larger slice contains unrelated or trailing bytes.
///
/// [`with_byte_range`]: Self::with_byte_range
///
/// # Example
///
/// ```rust
/// use vmnl_graphics::texture::{AlphaMode, ColorEncoding, PixelLayout, PixelOrigin,
///     PixelSource, TextureFormat, TextureUsageRequest};
///
/// let bytes = [0, 0, 0, 0, 255, 0, 0, 255];
/// let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &bytes)?
///     .with_byte_range(4..8)
///     .with_layout(PixelLayout::new(0, 4, PixelOrigin::TopLeft))
///     .with_encoding(ColorEncoding::Linear)
///     .with_alpha_mode(AlphaMode::Straight)
///     .with_usage(TextureUsageRequest::Automatic);
/// assert_eq!(source.extent(), [1, 1]);
/// assert_eq!(source.format(), TextureFormat::Rgba8Unorm);
/// assert_eq!(source.byte_range(), Some(4..8));
/// assert_eq!(source.encoding(), ColorEncoding::Linear);
/// assert_eq!(source.alpha_mode(), AlphaMode::Straight);
/// assert_eq!(source.layout().offset(), 0);
/// assert_eq!(source.layout().row_stride(), 4);
/// assert_eq!(source.layout().origin(), PixelOrigin::TopLeft);
/// assert_eq!(source.usage(), TextureUsageRequest::Automatic);
/// source.validate()?;
///
/// let prepared = source.prepare()?;
/// assert_eq!(prepared.extent(), [1, 1]);
/// assert_eq!(prepared.format(), TextureFormat::Rgba8Unorm);
/// assert_eq!(prepared.encoding(), ColorEncoding::Linear);
/// assert_eq!(prepared.alpha_mode(), AlphaMode::Straight);
/// assert_eq!(prepared.bytes(), &bytes[4..]);
/// assert_eq!(prepared.into_bytes(), bytes[4..]);
/// # Ok::<(), vmnl_graphics::VMNLError>(())
/// ```
#[derive(Debug)]
pub struct PixelSource<'a> {
    width: u32,
    height: u32,
    format: TextureFormat,
    bytes: &'a [u8],
    explicit_byte_range: Option<Range<usize>>,
    layout: PixelLayout,
    encoding: ColorEncoding,
    alpha_mode: AlphaMode,
    usage: TextureUsageRequest,
}

impl<'a> PixelSource<'a> {
    /// Creates a source whose default layout is tightly packed and top-left-origin.
    ///
    /// # Arguments
    ///
    /// - `width` supplies the source image width in texels.
    /// - `height` supplies the source image height in texels.
    /// - `format` describes the representation of the source texels.
    /// - `bytes` supplies the borrowed source pixels, including any padding described by a custom
    ///   layout.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` if the default RGBA8 row size overflows the host address space.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    pub fn new(
        width: u32,
        height: u32,
        format: TextureFormat,
        bytes: &'a [u8],
    ) -> VMNLResult<Self> {
        let row_stride = checked_row_bytes(width)?;

        Ok(Self {
            width,
            height,
            format,
            bytes,
            explicit_byte_range: None,
            layout: PixelLayout::new(0, row_stride, PixelOrigin::TopLeft),
            encoding: format.default_encoding(),
            alpha_mode: AlphaMode::Straight,
            usage: TextureUsageRequest::Automatic,
        })
    }

    /// Declares a half-open byte range in `bytes`; layout offsets are relative to this range.
    ///
    /// Extra bytes after the required pixel rows may be ignored within this
    /// explicit range. Bytes before or after the range are never read.
    ///
    /// # Arguments
    ///
    /// - `range` selects the half-open interval of the source slice available for pixel reads.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub fn with_byte_range(mut self, range: Range<usize>) -> Self {
        self.explicit_byte_range = Some(range);
        self
    }

    /// Replaces the source row layout.
    ///
    /// # Arguments
    ///
    /// - `layout` describes the first-row offset, stride, and origin of the source bytes.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn with_layout(mut self, layout: PixelLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Declares the channel encoding without converting supplied values.
    ///
    /// # Arguments
    ///
    /// - `encoding` declares how the color values in the source bytes are encoded.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn with_encoding(mut self, encoding: ColorEncoding) -> Self {
        self.encoding = encoding;
        self
    }

    /// Declares the supplied alpha representation without converting supplied values.
    ///
    /// # Arguments
    ///
    /// - `alpha_mode` declares whether the source color channels are premultiplied by alpha.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn with_alpha_mode(mut self, alpha_mode: AlphaMode) -> Self {
        self.alpha_mode = alpha_mode;
        self
    }

    /// Replaces the requested image usage policy.
    ///
    /// # Arguments
    ///
    /// - `usage` supplies the usage policy retained with the prepared pixel data.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn with_usage(mut self, usage: TextureUsageRequest) -> Self {
        self.usage = usage;
        self
    }

    /// Returns the image extent in pixels.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn extent(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    /// Returns the requested format.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn format(&self) -> TextureFormat {
        self.format
    }

    /// Returns the optional half-open byte range in the supplied slice.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub fn byte_range(&self) -> Option<Range<usize>> {
        self.explicit_byte_range.clone()
    }

    /// Returns the requested color encoding.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn encoding(&self) -> ColorEncoding {
        self.encoding
    }

    /// Returns the requested alpha representation.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    /// Returns the input row layout.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn layout(&self) -> PixelLayout {
        self.layout
    }

    /// Returns the requested image usage policy.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    #[must_use]
    pub const fn usage(&self) -> TextureUsageRequest {
        self.usage
    }

    /// Validates dimensions, format declarations, row stride, and byte coverage.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for zero dimensions, conflicting declarations, invalid ranges,
    /// row strides, empty exact usage, or insufficient/ambiguous byte coverage.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    pub fn validate(&self) -> VMNLResult<()> {
        if self.width == 0 || self.height == 0 {
            return Err(invalid_state("pixel source dimensions must be nonzero"));
        }
        if self.encoding != self.format.default_encoding() {
            return Err(invalid_state(
                "pixel format and color encoding declarations conflict",
            ));
        }
        if let TextureUsageRequest::Exact(usage) = self.usage {
            if usage.is_empty() {
                return Err(invalid_state("exact texture usage must not be empty"));
            }
        }

        let row_bytes = checked_row_bytes(self.width)?;
        if self.layout.row_stride < row_bytes {
            return Err(invalid_state(
                "pixel row stride is smaller than one image row",
            ));
        }

        let rows_before_last = usize::try_from(self.height - 1)
            .map_err(|_| invalid_state("pixel height does not fit the host address space"))?;
        let required_end = self
            .layout
            .offset
            .checked_add(
                rows_before_last
                    .checked_mul(self.layout.row_stride)
                    .ok_or_else(|| invalid_state("pixel row range overflows address space"))?,
            )
            .and_then(|end| end.checked_add(row_bytes))
            .ok_or_else(|| invalid_state("pixel source range overflows address space"))?;
        let source_bytes = self.source_bytes()?;

        if required_end > source_bytes.len() {
            return Err(invalid_state(format!(
                "pixel source requires {required_end} bytes but its declared subrange has {}",
                source_bytes.len()
            )));
        }
        if self.explicit_byte_range.is_none() && required_end != source_bytes.len() {
            return Err(invalid_state(
                "trailing pixel bytes require an explicit source byte range",
            ));
        }

        Ok(())
    }

    /// Copies source rows into tightly packed RGBA8 bytes with a top-left origin.
    ///
    /// This allocates one owned output buffer and preserves all channel values.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` for invalid descriptors or row/size arithmetic that overflows
    /// the host address space. Returns `TextureCpuAllocationFailed` if allocating the output fails.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self`].
    pub fn prepare(&self) -> VMNLResult<PreparedPixelData> {
        self.validate()?;

        let row_bytes = checked_row_bytes(self.width)?;
        let output_len =
            row_bytes
                .checked_mul(usize::try_from(self.height).map_err(|_| {
                    invalid_state("pixel height does not fit the host address space")
                })?)
                .ok_or_else(|| invalid_state("prepared pixel data size overflows address space"))?;
        let mut output = Vec::new();
        output.try_reserve_exact(output_len).map_err(|error| {
            VMNLError::new(VMNLErrorKind::TextureCpuAllocationFailed(error.to_string()))
        })?;
        output.resize(output_len, 0);

        let source_bytes = self.source_bytes()?;
        for output_row in 0..self.height {
            let source_row = match self.layout.origin {
                PixelOrigin::TopLeft => output_row,
                PixelOrigin::BottomLeft => self.height - 1 - output_row,
            };
            let source_start = self
                .layout
                .offset
                .checked_add(
                    usize::try_from(source_row)
                        .map_err(|_| invalid_state("pixel row does not fit address space"))?
                        .checked_mul(self.layout.row_stride)
                        .ok_or_else(|| invalid_state("pixel row range overflows address space"))?,
                )
                .ok_or_else(|| invalid_state("pixel row offset overflows address space"))?;
            let source_end = source_start
                .checked_add(row_bytes)
                .ok_or_else(|| invalid_state("pixel row end overflows address space"))?;
            let destination_start = usize::try_from(output_row)
                .map_err(|_| invalid_state("pixel row does not fit address space"))?
                .checked_mul(row_bytes)
                .ok_or_else(|| invalid_state("prepared pixel row offset overflows"))?;
            let destination_end = destination_start
                .checked_add(row_bytes)
                .ok_or_else(|| invalid_state("prepared pixel row end overflows"))?;
            let source = source_bytes
                .get(source_start..source_end)
                .ok_or_else(|| invalid_state("validated pixel source row is unavailable"))?;
            let destination = output
                .get_mut(destination_start..destination_end)
                .ok_or_else(|| invalid_state("prepared pixel destination row is unavailable"))?;
            destination.copy_from_slice(source);
        }

        Ok(PreparedPixelData {
            width: self.width,
            height: self.height,
            format: self.format,
            encoding: self.encoding,
            alpha_mode: self.alpha_mode,
            bytes: output,
        })
    }

    fn source_bytes(&self) -> VMNLResult<&'a [u8]> {
        let Some(range) = &self.explicit_byte_range else {
            return Ok(self.bytes);
        };
        self.bytes
            .get(range.clone())
            .ok_or_else(|| invalid_state("pixel source byte range is outside the supplied slice"))
    }
}

/// Owned, packed RGBA8 pixels normalized to a top-left origin.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedPixelData {
    width: u32,
    height: u32,
    format: TextureFormat,
    encoding: ColorEncoding,
    alpha_mode: AlphaMode,
    bytes: Vec<u8>,
}

impl PreparedPixelData {
    pub(crate) fn new(
        width: u32,
        height: u32,
        format: TextureFormat,
        encoding: ColorEncoding,
        alpha_mode: AlphaMode,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            width,
            height,
            format,
            encoding,
            alpha_mode,
            bytes,
        }
    }

    /// Returns the image extent in pixels.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn extent(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    /// Returns the RGBA8 format to request for sampling.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn format(&self) -> TextureFormat {
        self.format
    }

    /// Returns the declared channel encoding; pixel values are not converted.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn encoding(&self) -> ColorEncoding {
        self.encoding
    }

    /// Returns the declared alpha representation; pixel values are not converted.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub const fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    /// Returns tightly packed, top-left-origin RGBA8 bytes.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the owned packed bytes and consumes this CPU-side image.
    ///
    /// # Example
    ///
    /// See the workflow on [`PixelSource`].
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn checked_row_bytes(width: u32) -> VMNLResult<usize> {
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(TextureFormat::bytes_per_pixel()))
        .ok_or_else(|| invalid_state("pixel row byte count overflows address space"))
}

pub(crate) fn invalid_state(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::InvalidState(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_source_accepts_exact_2_by_2_padded_fixture_and_excludes_padding() -> VMNLResult<()> {
        let bytes = [
            99, 99, 99, 99, 1, 2, 3, 4, 5, 6, 7, 8, 250, 251, 252, 253, 9, 10, 11, 12, 13, 14, 15,
            16,
        ];
        let source = PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &bytes)?
            .with_byte_range(0..24)
            .with_layout(PixelLayout::new(4, 12, PixelOrigin::TopLeft));

        let prepared = source.prepare()?;
        assert_eq!(prepared.extent(), [2, 2]);
        assert_eq!(
            prepared.bytes(),
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
        Ok(())
    }

    #[test]
    fn pixel_source_rejects_one_byte_less_than_required() -> VMNLResult<()> {
        let bytes = [0; 23];
        let source = PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &bytes)?
            .with_byte_range(0..23)
            .with_layout(PixelLayout::new(4, 12, PixelOrigin::TopLeft));

        assert!(matches!(
            source.validate(),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message)
                if message.contains("requires 24 bytes"))
        ));
        Ok(())
    }

    #[test]
    fn pixel_source_reverses_bottom_left_rows() -> VMNLResult<()> {
        let bytes = [
            9, 10, 11, 12, 13, 14, 15, 16, 90, 91, 92, 93, 1, 2, 3, 4, 5, 6, 7, 8,
        ];
        let source = PixelSource::new(2, 2, TextureFormat::Rgba8Unorm, &bytes)?
            .with_byte_range(0..20)
            .with_layout(PixelLayout::new(0, 12, PixelOrigin::BottomLeft));

        let prepared = source.prepare()?;
        assert_eq!(
            prepared.bytes(),
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
        Ok(())
    }

    #[test]
    fn pixel_source_requires_explicit_subrange_for_trailing_bytes() -> VMNLResult<()> {
        let bytes = [0; 9];
        let source = PixelSource::new(2, 1, TextureFormat::Rgba8Unorm, &bytes)?;
        assert!(matches!(
            source.validate(),
            Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(message)
                if message.contains("explicit source byte range"))
        ));
        Ok(())
    }

    #[test]
    fn pixel_rect_checks_nonempty_extent_overflow_and_bounds() -> VMNLResult<()> {
        assert!(PixelRect::new(0, 0, 0, 1).is_err());
        assert!(PixelRect::new(u32::MAX, 0, 1, 1).is_err());

        let rect = PixelRect::new(1, 2, 3, 4)?;
        assert!(rect.validate_within(4, 6).is_ok());
        assert!(rect.validate_within(3, 6).is_err());
        Ok(())
    }

    #[test]
    fn pixel_source_rejects_conflicting_format_encoding_and_empty_exact_usage() -> VMNLResult<()> {
        let bytes = [0; 4];
        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Srgb, &bytes)?
            .with_encoding(ColorEncoding::Linear);
        assert!(source.validate().is_err());

        let source = PixelSource::new(1, 1, TextureFormat::Rgba8Unorm, &bytes)?
            .with_usage(TextureUsageRequest::Exact(TextureUsage::default()));
        assert!(source.validate().is_err());
        Ok(())
    }
}
