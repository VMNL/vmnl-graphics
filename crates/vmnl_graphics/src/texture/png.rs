// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Static PNG decoding into packed top-left-origin RGBA8 pixels.

use std::io::Cursor;

use png::{BitDepth, ColorType, Decoder, Info, Transformations};

use super::pixel::{AlphaMode, ColorEncoding, PreparedPixelData, TextureFormat};
use crate::{VMNLError, VMNLErrorKind, VMNLResult};

const PNG_DECODER_LIMIT_BYTES: usize = 64 * 1024 * 1024;

/// Optional declarations for decoding a static PNG.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PngDecodeOptions {
    color_encoding: Option<ColorEncoding>,
    alpha_mode: Option<AlphaMode>,
}

impl PngDecodeOptions {
    /// Overrides the default encoding only for an untagged PNG.
    #[must_use]
    pub const fn with_encoding(mut self, encoding: ColorEncoding) -> Self {
        self.color_encoding = Some(encoding);
        self
    }

    /// Declares the expected alpha representation.
    #[must_use]
    pub const fn with_alpha_mode(mut self, alpha_mode: AlphaMode) -> Self {
        self.alpha_mode = Some(alpha_mode);
        self
    }

    /// Returns the optional encoding declaration.
    #[must_use]
    pub const fn encoding(self) -> Option<ColorEncoding> {
        self.color_encoding
    }

    /// Returns the optional alpha declaration.
    #[must_use]
    pub const fn alpha_mode(self) -> Option<AlphaMode> {
        self.alpha_mode
    }

    /// Decodes a static PNG to tightly packed RGBA8 bytes with a top-left origin.
    ///
    /// Supported source samples are grayscale type 0 at 1, 2, 4, or 8 bits;
    /// truecolor type 2 at 8 bits; indexed type 3 at 1, 2, 4, or 8 bits; grayscale
    /// alpha type 4 at 8 bits; and truecolor alpha type 6 at 8 bits. Palette and
    /// low-bit grayscale samples are expanded to 8-bit channels. Pixel values are
    /// not gamma-converted, profile-converted, premultiplied, or otherwise changed.
    ///
    /// The default encoding is sRGB. A `sRGB` chunk may include the canonical
    /// `gAMA` and `cHRM` values; non-sRGB color metadata is rejected. Alpha-bearing
    /// files use straight alpha; files without alpha or `tRNS` use opaque alpha.
    /// Explicit declarations that conflict with those file properties fail.
    /// APNG and 16-bit samples are rejected. The decoder's internal working memory
    /// limit is 64 MiB; the returned RGBA8 allocation is checked separately.
    ///
    /// # Errors
    ///
    /// Returns [`VMNLErrorKind::TextureDecodeFailed`] for malformed PNG data or
    /// [`VMNLErrorKind::UnsupportedTextureSource`] for unsupported source features.
    /// Conflicting explicit declarations return [`VMNLErrorKind::InvalidState`].
    pub fn decode_png(self, bytes: &[u8]) -> VMNLResult<PreparedPixelData> {
        decode_png_impl(bytes, self)
    }
}

fn decode_png_impl(bytes: &[u8], options: PngDecodeOptions) -> VMNLResult<PreparedPixelData> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: PNG_DECODER_LIMIT_BYTES,
    });
    decoder.set_transformations(Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(decode_failed)?;
    let (width, height, source_color, source_depth, has_alpha, tagged_srgb) = {
        let info = reader.info();
        validate_source_info(info)?;
        (
            info.width,
            info.height,
            info.color_type,
            info.bit_depth,
            source_has_alpha(info),
            info.srgb.is_some(),
        )
    };

    if width == 0 || height == 0 {
        return Err(unsupported("PNG dimensions must be nonzero"));
    }
    validate_supported_sample_format(source_color, source_depth)?;

    let encoding = match (tagged_srgb, options.color_encoding) {
        (true, Some(encoding)) if encoding != ColorEncoding::Srgb => {
            return Err(invalid_state(
                "explicit PNG encoding conflicts with the sRGB metadata",
            ));
        }
        (true, _) | (false, None) => ColorEncoding::Srgb,
        (false, Some(encoding)) => encoding,
    };
    let alpha_mode = if has_alpha {
        if options
            .alpha_mode
            .is_some_and(|requested| requested != AlphaMode::Straight)
        {
            return Err(invalid_state(
                "PNG alpha samples are unassociated and require straight alpha",
            ));
        }
        AlphaMode::Straight
    } else {
        if options
            .alpha_mode
            .is_some_and(|requested| requested != AlphaMode::Opaque)
        {
            return Err(invalid_state(
                "PNG without alpha or tRNS requires opaque alpha",
            ));
        }
        AlphaMode::Opaque
    };

    let frame_buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| decode_failed_message("decoded frame size exceeds address space"))?;
    let mut frame_bytes = allocate_decode_buffer(frame_buffer_size)?;
    let output_info = reader.next_frame(&mut frame_bytes).map_err(decode_failed)?;
    if output_info.width != width || output_info.height != height {
        return Err(decode_failed_message(
            "static PNG decoder returned an unexpected frame extent",
        ));
    }
    if output_info.bit_depth != BitDepth::Eight {
        return Err(unsupported(
            "PNG channel expansion did not produce 8-bit samples",
        ));
    }

    let output_row_bytes = checked_output_row_bytes(width)?;
    let output_len =
        output_row_bytes
            .checked_mul(usize::try_from(height).map_err(|_| {
                decode_failed_message("PNG height does not fit the host address space")
            })?)
            .ok_or_else(|| decode_failed_message("RGBA8 image size overflows address space"))?;
    let mut rgba_bytes = allocate_decode_buffer(output_len)?;
    expand_rows_to_rgba8(
        &frame_bytes,
        &output_info,
        &mut rgba_bytes,
        output_row_bytes,
    )?;

    let format = match encoding {
        ColorEncoding::Linear => TextureFormat::Rgba8Unorm,
        ColorEncoding::Srgb => TextureFormat::Rgba8Srgb,
    };
    Ok(PreparedPixelData::new(
        width, height, format, encoding, alpha_mode, rgba_bytes,
    ))
}

fn validate_source_info(info: &Info<'_>) -> VMNLResult<()> {
    if info.animation_control.is_some() {
        return Err(unsupported("APNG animation is not supported"));
    }

    if info.icc_profile.is_some()
        || info.coding_independent_code_points.is_some()
        || info.mastering_display_color_volume.is_some()
        || info.content_light_level.is_some()
    {
        return Err(unsupported(
            "iCCP, cICP, mDCV, and cLLI color metadata are not supported",
        ));
    }

    if info.srgb.is_some() {
        if info
            .gama_chunk
            .is_some_and(|gamma| gamma.into_scaled() != 45_455)
        {
            return Err(unsupported("sRGB PNG has a noncanonical gAMA value"));
        }
        if info
            .chrm_chunk
            .is_some_and(|chromaticities| !is_canonical_srgb_chromaticities(chromaticities))
        {
            return Err(unsupported("sRGB PNG has noncanonical cHRM values"));
        }
    } else if info.gama_chunk.is_some() || info.chrm_chunk.is_some() {
        return Err(unsupported(
            "gAMA and cHRM metadata without an sRGB declaration are not supported",
        ));
    }

    if info.color_type == ColorType::Indexed {
        let palette = info
            .palette
            .as_deref()
            .ok_or_else(|| unsupported("indexed PNG has no palette"))?;
        if palette.is_empty() || palette.len() > 768 || palette.len() % 3 != 0 {
            return Err(unsupported("indexed PNG palette is malformed"));
        }
        let max_entries = match info.bit_depth {
            BitDepth::One => 2,
            BitDepth::Two => 4,
            BitDepth::Four => 16,
            BitDepth::Eight => 256,
            BitDepth::Sixteen => 0,
        };
        if palette.len() / 3 > max_entries {
            return Err(unsupported(
                "indexed PNG palette has more entries than its bit depth permits",
            ));
        }
    }

    Ok(())
}

fn is_canonical_srgb_chromaticities(value: png::SourceChromaticities) -> bool {
    let expected = png::SourceChromaticities {
        white: (
            png::ScaledFloat::from_scaled(31_270),
            png::ScaledFloat::from_scaled(32_900),
        ),
        red: (
            png::ScaledFloat::from_scaled(64_000),
            png::ScaledFloat::from_scaled(33_000),
        ),
        green: (
            png::ScaledFloat::from_scaled(30_000),
            png::ScaledFloat::from_scaled(60_000),
        ),
        blue: (
            png::ScaledFloat::from_scaled(15_000),
            png::ScaledFloat::from_scaled(6_000),
        ),
    };
    value == expected
}

fn source_has_alpha(info: &Info<'_>) -> bool {
    matches!(info.color_type, ColorType::GrayscaleAlpha | ColorType::Rgba) || info.trns.is_some()
}

fn validate_supported_sample_format(color_type: ColorType, bit_depth: BitDepth) -> VMNLResult<()> {
    let supported = match color_type {
        ColorType::Grayscale => matches!(
            bit_depth,
            BitDepth::One | BitDepth::Two | BitDepth::Four | BitDepth::Eight
        ),
        ColorType::Rgb | ColorType::GrayscaleAlpha | ColorType::Rgba => {
            bit_depth == BitDepth::Eight
        }
        ColorType::Indexed => matches!(
            bit_depth,
            BitDepth::One | BitDepth::Two | BitDepth::Four | BitDepth::Eight
        ),
    };
    if supported {
        Ok(())
    } else {
        Err(unsupported(format!(
            "PNG color type {color_type:?} at {bit_depth:?} is not supported"
        )))
    }
}

fn checked_output_row_bytes(width: u32) -> VMNLResult<usize> {
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| decode_failed_message("RGBA8 row size overflows address space"))
}

fn allocate_decode_buffer(size: usize) -> VMNLResult<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|error| {
        VMNLError::new(VMNLErrorKind::TextureCpuAllocationFailed(error.to_string()))
    })?;
    bytes.resize(size, 0);
    Ok(bytes)
}

fn expand_rows_to_rgba8(
    frame_bytes: &[u8],
    info: &png::OutputInfo,
    rgba_bytes: &mut [u8],
    rgba_row_bytes: usize,
) -> VMNLResult<()> {
    let width = usize::try_from(info.width)
        .map_err(|_| decode_failed_message("PNG width does not fit the host address space"))?;
    let input_channels = match info.color_type {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => {
            return Err(unsupported(
                "indexed PNG was not expanded to RGB or RGBA channels",
            ));
        }
    };
    let input_row_bytes = width
        .checked_mul(input_channels)
        .ok_or_else(|| decode_failed_message("decoded PNG row size overflows address space"))?;
    if info.line_size < input_row_bytes {
        return Err(decode_failed_message(
            "decoded PNG row is smaller than its channel data",
        ));
    }

    for row in 0..info.height {
        let input_start = usize::try_from(row)
            .map_err(|_| decode_failed_message("PNG row does not fit address space"))?
            .checked_mul(info.line_size)
            .ok_or_else(|| decode_failed_message("decoded PNG row offset overflows"))?;
        let input_end = input_start
            .checked_add(input_row_bytes)
            .ok_or_else(|| decode_failed_message("decoded PNG row end overflows"))?;
        let output_start = usize::try_from(row)
            .map_err(|_| decode_failed_message("PNG row does not fit address space"))?
            .checked_mul(rgba_row_bytes)
            .ok_or_else(|| decode_failed_message("RGBA8 row offset overflows"))?;
        let output_end = output_start
            .checked_add(rgba_row_bytes)
            .ok_or_else(|| decode_failed_message("RGBA8 row end overflows"))?;
        let input = frame_bytes
            .get(input_start..input_end)
            .ok_or_else(|| decode_failed_message("decoded PNG row is unavailable"))?;
        let output = rgba_bytes
            .get_mut(output_start..output_end)
            .ok_or_else(|| decode_failed_message("RGBA8 output row is unavailable"))?;

        match info.color_type {
            ColorType::Grayscale => {
                for (gray, pixel) in input.iter().zip(output.chunks_exact_mut(4)) {
                    pixel.copy_from_slice(&[*gray, *gray, *gray, u8::MAX]);
                }
            }
            ColorType::GrayscaleAlpha => {
                for (source, pixel) in input.chunks_exact(2).zip(output.chunks_exact_mut(4)) {
                    pixel.copy_from_slice(&[source[0], source[0], source[0], source[1]]);
                }
            }
            ColorType::Rgb => {
                for (source, pixel) in input.chunks_exact(3).zip(output.chunks_exact_mut(4)) {
                    pixel.copy_from_slice(&[source[0], source[1], source[2], u8::MAX]);
                }
            }
            ColorType::Rgba => output.copy_from_slice(input),
            ColorType::Indexed => {
                return Err(unsupported(
                    "indexed PNG was not expanded to RGB or RGBA channels",
                ));
            }
        }
    }

    Ok(())
}

fn decode_failed(error: impl std::fmt::Display) -> VMNLError {
    decode_failed_message(error.to_string())
}

fn decode_failed_message(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::TextureDecodeFailed(message.into()))
}

fn unsupported(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::UnsupportedTextureSource(message.into()))
}

fn invalid_state(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::InvalidState(message.into()))
}

#[cfg(test)]
#[path = "png_tests.rs"]
mod tests;
