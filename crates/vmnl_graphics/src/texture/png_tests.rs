// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Deterministic PNG fixtures for supported formats and rejected metadata.

use std::error::Error;

use png::{BitDepth, ColorType, Encoder, SrgbRenderingIntent};

use super::*;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct PngFixture {
    color_type: ColorType,
    bit_depth: BitDepth,
    data: Vec<u8>,
    palette: Option<Vec<u8>>,
    transparency: Option<Vec<u8>>,
}

impl PngFixture {
    fn new(color_type: ColorType, bit_depth: BitDepth, data: &[u8]) -> Self {
        Self {
            color_type,
            bit_depth,
            data: data.to_vec(),
            palette: None,
            transparency: None,
        }
    }

    fn with_palette(mut self, palette: &[u8]) -> Self {
        self.palette = Some(palette.to_vec());
        self
    }

    fn with_transparency(mut self, transparency: &[u8]) -> Self {
        self.transparency = Some(transparency.to_vec());
        self
    }
}

fn encode_png(fixture: PngFixture) -> Result<Vec<u8>, png::EncodingError> {
    encode_png_with(fixture, |_| Ok(()))
}

fn encode_png_with(
    fixture: PngFixture,
    configure: impl FnOnce(&mut Encoder<'_, &mut Vec<u8>>) -> Result<(), png::EncodingError>,
) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(fixture.color_type);
        encoder.set_depth(fixture.bit_depth);
        if let Some(palette) = fixture.palette {
            encoder.set_palette(palette);
        }
        if let Some(transparency) = fixture.transparency {
            encoder.set_trns(transparency);
        }
        configure(&mut encoder)?;
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&fixture.data)?;
        writer.finish()?;
    }
    Ok(bytes)
}

#[test]
fn decoder_expands_supported_grayscale_depths() -> TestResult {
    for (bit_depth, packed, expected_gray) in [
        (BitDepth::One, 0x80, 255),
        (BitDepth::Two, 0x80, 170),
        (BitDepth::Four, 0xf0, 255),
        (BitDepth::Eight, 23, 23),
    ] {
        let png_bytes = encode_png(PngFixture::new(ColorType::Grayscale, bit_depth, &[packed]))?;
        let decoded = PngDecodeOptions::default().decode_png(&png_bytes)?;
        assert_eq!(
            decoded.bytes(),
            &[expected_gray, expected_gray, expected_gray, u8::MAX]
        );
    }
    Ok(())
}

#[test]
fn decoder_expands_indexed_depths_and_palette_values() -> TestResult {
    let fixtures = [
        (
            BitDepth::One,
            vec![0x80],
            vec![1, 2, 3, 4, 5, 6],
            [4, 5, 6, 255],
        ),
        (
            BitDepth::Two,
            vec![0x40],
            vec![0, 0, 0, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            [4, 5, 6, 255],
        ),
        (
            BitDepth::Four,
            vec![0x10],
            (0_u8..48).collect(),
            [3, 4, 5, 255],
        ),
        (
            BitDepth::Eight,
            vec![1],
            vec![1, 2, 3, 4, 5, 6],
            [4, 5, 6, 255],
        ),
    ];

    for (bit_depth, data, palette, expected) in fixtures {
        let fixture = PngFixture::new(ColorType::Indexed, bit_depth, &data).with_palette(&palette);
        let png_bytes = encode_png(fixture)?;
        let decoded = PngDecodeOptions::default().decode_png(&png_bytes)?;
        assert_eq!(decoded.bytes(), expected);
    }
    Ok(())
}

#[test]
fn decoder_expands_truecolor_and_alpha_channels() -> TestResult {
    let fixtures = [
        (
            PngFixture::new(ColorType::Rgb, BitDepth::Eight, &[1, 2, 3]),
            [1, 2, 3, 255],
        ),
        (
            PngFixture::new(ColorType::GrayscaleAlpha, BitDepth::Eight, &[8, 9]),
            [8, 8, 8, 9],
        ),
        (
            PngFixture::new(ColorType::Rgba, BitDepth::Eight, &[1, 2, 3, 4]),
            [1, 2, 3, 4],
        ),
    ];
    for (fixture, expected) in fixtures {
        let png_bytes = encode_png(fixture)?;
        let decoded = PngDecodeOptions::default().decode_png(&png_bytes)?;
        assert_eq!(decoded.bytes(), expected);
    }
    Ok(())
}

#[test]
fn decoder_handles_trns_and_declares_alpha_representation() -> TestResult {
    let indexed = PngFixture::new(ColorType::Indexed, BitDepth::Eight, &[0])
        .with_palette(&[10, 20, 30])
        .with_transparency(&[64]);
    let png_bytes = encode_png(indexed)?;
    let decoded = PngDecodeOptions::default().decode_png(&png_bytes)?;
    assert_eq!(decoded.bytes(), &[10, 20, 30, 64]);
    assert_eq!(decoded.alpha_mode(), AlphaMode::Straight);
    assert!(matches!(
        PngDecodeOptions::default()
            .with_alpha_mode(AlphaMode::Premultiplied)
            .decode_png(&png_bytes),
        Err(error) if matches!(error.kind(), VMNLErrorKind::InvalidState(_))
    ));

    let truecolor = PngFixture::new(ColorType::Rgb, BitDepth::Eight, &[1, 2, 3])
        .with_transparency(&[0, 1, 0, 2, 0, 3]);
    let png_bytes = encode_png(truecolor)?;
    let decoded = PngDecodeOptions::default().decode_png(&png_bytes)?;
    assert_eq!(decoded.bytes(), &[1, 2, 3, 0]);
    assert_eq!(decoded.alpha_mode(), AlphaMode::Straight);

    let opaque = encode_png(PngFixture::new(ColorType::Rgb, BitDepth::Eight, &[4, 5, 6]))?;
    let decoded = PngDecodeOptions::default().decode_png(&opaque)?;
    assert_eq!(decoded.alpha_mode(), AlphaMode::Opaque);
    assert!(PngDecodeOptions::default()
        .with_alpha_mode(AlphaMode::Straight)
        .decode_png(&opaque)
        .is_err());
    Ok(())
}

#[test]
fn decoder_uses_srgb_default_and_accepts_only_canonical_srgb_fallbacks() -> TestResult {
    let untagged = encode_png(PngFixture::new(
        ColorType::Rgba,
        BitDepth::Eight,
        &[1, 2, 3, 4],
    ))?;
    let default = PngDecodeOptions::default().decode_png(&untagged)?;
    assert_eq!(default.encoding(), ColorEncoding::Srgb);
    assert_eq!(default.format(), TextureFormat::Rgba8Srgb);

    let linear = PngDecodeOptions::default()
        .with_encoding(ColorEncoding::Linear)
        .decode_png(&untagged)?;
    assert_eq!(linear.encoding(), ColorEncoding::Linear);
    assert_eq!(linear.format(), TextureFormat::Rgba8Unorm);

    let tagged = encode_png_with(
        PngFixture::new(ColorType::Rgba, BitDepth::Eight, &[1, 2, 3, 4]),
        |encoder| {
            encoder.set_source_srgb(SrgbRenderingIntent::Perceptual);
            Ok(())
        },
    )?;
    assert_eq!(
        PngDecodeOptions::default().decode_png(&tagged)?.encoding(),
        ColorEncoding::Srgb
    );
    assert!(PngDecodeOptions::default()
        .with_encoding(ColorEncoding::Linear)
        .decode_png(&tagged)
        .is_err());

    let canonical_fallbacks = encode_png_with(
        PngFixture::new(ColorType::Rgba, BitDepth::Eight, &[1, 2, 3, 4]),
        |encoder| {
            encoder.set_source_srgb(SrgbRenderingIntent::Perceptual);
            encoder.set_source_gamma(png::ScaledFloat::from_scaled(45_455));
            encoder.set_source_chromaticities(srgb_chromaticities());
            Ok(())
        },
    )?;
    assert!(PngDecodeOptions::default()
        .decode_png(&canonical_fallbacks)
        .is_ok());

    let non_srgb_gamma = encode_png_with(
        PngFixture::new(ColorType::Rgba, BitDepth::Eight, &[1, 2, 3, 4]),
        |encoder| {
            encoder.set_source_gamma(png::ScaledFloat::from_scaled(45_000));
            Ok(())
        },
    )?;
    assert!(matches!(
        PngDecodeOptions::default().decode_png(&non_srgb_gamma),
        Err(error) if matches!(error.kind(), VMNLErrorKind::UnsupportedTextureSource(_))
    ));

    let noncanonical_tagged =
        insert_png_chunk_before_idat(&tagged, *b"gAMA", &45_000_u32.to_be_bytes());
    assert!(noncanonical_tagged
        .is_some_and(|bytes| { PngDecodeOptions::default().decode_png(&bytes).is_err() }));
    Ok(())
}

#[test]
fn decoder_rejects_16_bit_samples_and_apng() -> TestResult {
    let sixteen_bit = encode_png(PngFixture::new(
        ColorType::Grayscale,
        BitDepth::Sixteen,
        &[0, 255],
    ))?;
    assert!(matches!(
        PngDecodeOptions::default().decode_png(&sixteen_bit),
        Err(error) if matches!(error.kind(), VMNLErrorKind::UnsupportedTextureSource(_))
    ));

    let animated = encode_png_with(
        PngFixture::new(ColorType::Rgba, BitDepth::Eight, &[1, 2, 3, 4]),
        |encoder| encoder.set_animated(1, 0),
    )?;
    assert!(matches!(
        PngDecodeOptions::default().decode_png(&animated),
        Err(error) if matches!(error.kind(), VMNLErrorKind::UnsupportedTextureSource(_))
    ));
    Ok(())
}

#[test]
fn decoder_rejects_malformed_data_palettes_and_color_metadata() -> TestResult {
    assert!(matches!(
        PngDecodeOptions::default().decode_png(&[1, 2, 3]),
        Err(error) if matches!(error.kind(), VMNLErrorKind::TextureDecodeFailed(_))
    ));

    let indexed = encode_png(
        PngFixture::new(ColorType::Indexed, BitDepth::Eight, &[0])
            .with_palette(&[1, 2, 3, 4, 5, 6]),
    )?;
    let Some(malformed_palette) = replace_png_chunk(&indexed, *b"PLTE", &[1, 2, 3, 4]) else {
        return Err(std::io::Error::other("indexed fixture has no PLTE chunk").into());
    };
    assert!(PngDecodeOptions::default()
        .decode_png(&malformed_palette)
        .is_err());

    let rgba = [1, 2, 3, 4];
    let mut icc_info = png::Info::with_size(1, 1);
    icc_info.color_type = ColorType::Rgba;
    icc_info.bit_depth = BitDepth::Eight;
    icc_info.icc_profile = Some(std::borrow::Cow::Borrowed(b"fixture profile"));
    let mut icc_png = Vec::new();
    let icc_encoder = Encoder::with_info(&mut icc_png, icc_info)?;
    let mut icc_writer = icc_encoder.write_header()?;
    icc_writer.write_image_data(&rgba)?;
    icc_writer.finish()?;
    assert!(matches!(
        PngDecodeOptions::default().decode_png(&icc_png),
        Err(error) if matches!(error.kind(), VMNLErrorKind::UnsupportedTextureSource(_))
    ));

    let chunks = [
        (*b"cICP", &[1, 13, 0, 1][..]),
        (*b"mDCV", &[0; 24][..]),
        (*b"cLLI", &[0; 8][..]),
        (*b"cHRM", &[0; 32][..]),
    ];
    for (chunk_type, chunk_data) in chunks {
        let png_bytes = encode_png_with_chunk(chunk_type, chunk_data, rgba)?;
        let decoded = PngDecodeOptions::default().decode_png(&png_bytes);
        assert!(
            matches!(
                &decoded,
                Err(error) if matches!(error.kind(), VMNLErrorKind::UnsupportedTextureSource(_))
            ),
            "metadata chunk {chunk_type:?} returned {decoded:?}"
        );
    }
    Ok(())
}

fn srgb_chromaticities() -> png::SourceChromaticities {
    png::SourceChromaticities {
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
    }
}

fn encode_png_with_chunk(
    chunk_type: [u8; 4],
    chunk_data: &[u8],
    pixel: [u8; 4],
) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_chunk(png::chunk::ChunkType(chunk_type), chunk_data)?;
        writer.write_image_data(&pixel)?;
        writer.finish()?;
    }
    Ok(bytes)
}

fn insert_png_chunk_before_idat(
    png_bytes: &[u8],
    chunk_type: [u8; 4],
    chunk_data: &[u8],
) -> Option<Vec<u8>> {
    let mut output = png_bytes.get(..8)?.to_vec();
    let mut cursor = 8;
    let mut inserted = false;
    while cursor < png_bytes.len() {
        let (kind, chunk_end) = png_chunk_bounds(png_bytes, cursor)?;
        if kind == *b"IDAT" && !inserted {
            append_png_chunk(&mut output, chunk_type, chunk_data)?;
            inserted = true;
        }
        output.extend_from_slice(png_bytes.get(cursor..chunk_end)?);
        cursor = chunk_end;
    }
    inserted.then_some(output)
}

fn replace_png_chunk(png_bytes: &[u8], chunk_type: [u8; 4], replacement: &[u8]) -> Option<Vec<u8>> {
    let mut output = png_bytes.get(..8)?.to_vec();
    let mut cursor = 8;
    let mut replaced = false;
    while cursor < png_bytes.len() {
        let (kind, chunk_end) = png_chunk_bounds(png_bytes, cursor)?;
        if kind == chunk_type {
            let replacement_len = u32::try_from(replacement.len()).ok()?;
            output.extend_from_slice(&replacement_len.to_be_bytes());
            output.extend_from_slice(&kind);
            output.extend_from_slice(replacement);
            let mut checksum_data = Vec::new();
            checksum_data
                .try_reserve_exact(4 + replacement.len())
                .ok()?;
            checksum_data.extend_from_slice(&kind);
            checksum_data.extend_from_slice(replacement);
            output.extend_from_slice(&crc32(&checksum_data).to_be_bytes());
            replaced = true;
        } else {
            output.extend_from_slice(png_bytes.get(cursor..chunk_end)?);
        }
        cursor = chunk_end;
    }
    replaced.then_some(output)
}

fn png_chunk_bounds(png_bytes: &[u8], cursor: usize) -> Option<([u8; 4], usize)> {
    let header_end = cursor.checked_add(8)?;
    let header = png_bytes.get(cursor..header_end)?;
    let length_bytes: [u8; 4] = header.get(..4)?.try_into().ok()?;
    let length = usize::try_from(u32::from_be_bytes(length_bytes)).ok()?;
    let kind = header.get(4..8)?.try_into().ok()?;
    let data_end = header_end.checked_add(length)?;
    let chunk_end = data_end.checked_add(4)?;
    png_bytes.get(cursor..chunk_end)?;
    Some((kind, chunk_end))
}

fn append_png_chunk(output: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) -> Option<()> {
    let length = u32::try_from(data.len()).ok()?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&chunk_type);
    output.extend_from_slice(data);
    let mut checksum_data = Vec::new();
    checksum_data.try_reserve_exact(4 + data.len()).ok()?;
    checksum_data.extend_from_slice(&chunk_type);
    checksum_data.extend_from_slice(data);
    output.extend_from_slice(&crc32(&checksum_data).to_be_bytes());
    Some(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
