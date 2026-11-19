# Texture pixel descriptors

## Public path and maturity

Import types from `vmnl_graphics::texture`. Pixel descriptors validate and
normalize CPU data; [`Texture2DBuilder`](gpu_resources.md#texture2d-and-texture2dbuilder)
uses them as the source for GPU resource construction.

## Purpose and use cases

Describe caller-owned RGBA8 bytes, validate a byte range and row layout, then
produce an owned tightly packed buffer with a canonical top-left origin.

## Public API

### TextureFormat

`Rgba8Unorm` requests four 8-bit channels sampled as linear values or data.
`Rgba8Srgb` requests native sRGB sampling for RGB. Both contain four bytes per
pixel.

### ColorEncoding

`Linear` and `Srgb` declare RGB interpretation. They do not convert channel
values. The declaration must match `TextureFormat`.

### AlphaMode

`Straight`, `Premultiplied`, and `Opaque` declare the supplied alpha
representation. Pixel preparation never changes alpha or RGB values.

### PixelOrigin

`TopLeft` and `BottomLeft` identify the logical edge represented by the first
input row. The default is `TopLeft`.

### PixelLayout

`PixelLayout::new(offset, row_stride, origin)` stores a byte offset relative to
the declared byte subrange, bytes between row starts, and the source origin.
Getters are `offset`, `row_stride`, and `origin`.

### PixelRect

`PixelRect::new(x, y, width, height)` creates a non-empty half-open rectangle
in top-left-origin pixel coordinates. `validate_within(image_width,
image_height)` checks its bounds. Getters return each coordinate or extent.

### TextureUsage

The closed usage set provides `SAMPLED`, `TRANSFER_DESTINATION`,
`TRANSFER_SOURCE`, and `INITIAL_UPLOAD`. Combine values with `union`; inspect
them with `contains` or `is_empty`. `INITIAL_UPLOAD` is sampled plus transfer
destination and deliberately omits transfer source.

### TextureUsageRequest

`Automatic` selects sampled plus transfer-destination usage for the initial
upload. `Exact(TextureUsage)` requests the exact set; the GPU builder rejects
an initial upload that omits transfer-destination usage and does not add usage
bits. Exact empty usage is invalid.

### PixelSource

`PixelSource::new(width, height, format, bytes)` defaults to tightly packed
top-left rows, the encoding associated with the format, straight alpha, and
automatic usage. `with_byte_range`, `with_layout`, `with_encoding`,
`with_alpha_mode`, and `with_usage` replace those requests. Read requested
values with `extent`, `format`, `byte_range`, `encoding`, `alpha_mode`, `layout`,
and `usage`. `byte_range` is `None` until a range is explicitly supplied.

`validate` checks positive dimensions, a matching format/encoding pair, an
exact usage request, row stride, checked arithmetic, and byte coverage. The
required end is `offset + (height - 1) * row_stride + row_bytes`, with checked
operations. A row stride must cover a complete row. If no explicit byte range
was supplied, the slice length must equal the required end. An explicit
half-open byte range may include ignored bytes after the required end. Bytes
outside that range are never read.

`prepare` copies only texel bytes, reverses bottom-left rows when needed, and
returns `PreparedPixelData`. It preserves declarations and channel values.

### PreparedPixelData

This owned value contains packed top-left-origin RGBA8 pixels and their extent,
format, encoding, and alpha declaration. `bytes` borrows the packed data;
`into_bytes` transfers its `Vec<u8>`.

## Construction, defaults, and validation

The constructor computes the default RGBA8 row stride with checked arithmetic.
`validate` and `prepare` reject zero dimensions, conflicting format/encoding,
empty exact usage, invalid ranges, short rows, overflow, and undersized data.
No device capability is inferred here.

## Units, coordinates, and valid ranges

Dimensions, rectangle coordinates, and row counts use pixels. Offsets and row
strides use bytes. Rectangles use top-left origin and half-open ends.

## Ownership, lifecycle, and threading

`PixelSource` borrows caller bytes for validation/preparation. `PreparedPixelData`
owns its copied bytes and is independent of the original source. These CPU
values have no thread-affinity requirement.

## Errors, panics, and failure conditions

Invalid descriptors return `VMNLErrorKind::InvalidState`. CPU allocation
failures return `TextureCpuAllocationFailed`. The preparation path uses checked
range arithmetic and returns errors for unavailable rows rather than indexing
outside validated slices.

## Allocation, transfers, synchronization, and GPU cost

`validate` does not allocate. `prepare` allocates one tightly packed RGBA8
buffer and copies every pixel once; it may also reverse row order. It submits
no GPU work and performs no synchronization.

## Platform, Vulkan, and display constraints

CPU validation is headless. A device later resolves format, usage, dimensions,
and upload alignment support.

## Example and related types

```rust
# extern crate vmnl_graphics;
# fn main() -> vmnl_graphics::VMNLResult<()> {
use vmnl_graphics::texture::{PixelLayout, PixelOrigin, PixelSource, TextureFormat};

let bytes = [255_u8, 0, 0, 255];
let prepared = PixelSource::new(1, 1, TextureFormat::Rgba8Srgb, &bytes)?
    .with_layout(PixelLayout::new(0, 4, PixelOrigin::TopLeft))
    .prepare()?;
assert_eq!(prepared.bytes(), &bytes);
# Ok(())
# }
```

Related: [PNG decoding](png_decode.md) and [sampler configuration](sampler_config.md).
