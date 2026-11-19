# Static PNG decoding

## Public path and maturity

`PngDecodeOptions` and its `decode_png` method are available through
`vmnl_graphics::texture`. They produce CPU-owned prepared pixels and do not
create a GPU image.

## Purpose and use cases

Decode supported in-memory static PNG bytes into packed RGBA8 suitable for a
later texture upload.

## Public API

### PngDecodeOptions

`PngDecodeOptions::default()` leaves color and alpha declarations automatic.
`with_encoding` can override the sRGB default only when the file has no color
metadata. `with_alpha_mode` must match the source alpha representation.
`encoding` and `alpha_mode` return the optional declarations.

### `PngDecodeOptions::decode_png`

`options.decode_png(bytes)` returns `PreparedPixelData`. Untagged input defaults
to sRGB. A tagged sRGB image remains sRGB; an explicit conflicting encoding
fails. PNG alpha samples are unassociated, so alpha-bearing images default to
`Straight`. Images without alpha or `tRNS` default to `Opaque`. Conflicting
explicit alpha modes fail.

Supported input matrix:

| PNG color type | Supported bit depths |
|---|---|
| Grayscale | 1, 2, 4, 8 |
| Truecolor | 8 |
| Indexed | 1, 2, 4, 8 |
| Grayscale with alpha | 8 |
| Truecolor with alpha | 8 |

Palette and low-bit grayscale values expand to 8-bit channels; `tRNS` expands
to straight alpha. The decoder rejects 16-bit samples, APNG, malformed indexed
palettes, noncanonical or untagged `gAMA`/`cHRM`, and `iCCP`, `cICP`, `mDCV`, or
`cLLI` metadata. `sRGB` may be accompanied by canonical `gAMA=45455` and
canonical sRGB `cHRM` values (white `(31270,32900)`, red `(64000,33000)`, green
`(30000,60000)`, blue `(15000,6000)`). No profile, gamma, alpha, or other pixel-value
conversion occurs beyond channel expansion and adding opaque alpha where the
input has none.

## Construction, defaults, and validation

The decoder checks source type, bit depth, palette shape and size, metadata,
dimensions, and allocation sizes before returning pixels. It applies the PNG
decoder's 64 MiB internal working-memory limit. The returned RGBA8 buffer is
separately allocated with fallible reservation.

## Units, coordinates, and valid ranges

Output dimensions are pixels. The returned byte order is RGBA, row stride is
`width * 4`, and the first row is the logical top row.

## Ownership, lifecycle, and threading

Input bytes are borrowed only for the duration of the call. `PreparedPixelData`
owns the result and has no thread-affinity requirement.

## Errors, panics, and failure conditions

Malformed PNG data returns `TextureDecodeFailed`; unsupported source features
return `UnsupportedTextureSource`; conflicting options return `InvalidState`;
CPU buffer allocation returns `TextureCpuAllocationFailed`. Decoder error
types do not appear in VMNL's public signatures.

## Allocation, transfers, synchronization, and GPU cost

Decoding allocates decoder working buffers and one owned RGBA8 output buffer.
It submits no GPU work and performs no synchronization. It does not retain
compressed source bytes.

## Platform, Vulkan, and display constraints

Decoding is CPU-only and does not require Vulkan, GLFW, a window, or a display.

## Example and related types

```rust
# extern crate vmnl_graphics;
use vmnl_graphics::{texture::{PngDecodeOptions, PreparedPixelData}, VMNLResult};

fn decode(png_bytes: &[u8]) -> VMNLResult<PreparedPixelData> {
    PngDecodeOptions::default().decode_png(png_bytes)
}
```

Related: [pixel descriptors](descriptors.md) and [`VMNLErrorKind`](../errors/vmnl_error_kind.md).
