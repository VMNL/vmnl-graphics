# Defaults matrix

| Surface | Default |
|---|---|
| `BufferMemoryPreference` | `Device` |
| `ContextBuilder` / `DeviceConfig` | automatic compatible GPU/graphics queue selection; no optional feature request |
| `TextureDefaults` / `ColorBlendOverrides` | every field unset; resolve local request, context request, then standard default |
| `Rgba` | `(0, 0, 0, 0)`; named colors are opaque |
| `Vector2f`, `Vertex2D` | Component/field zero defaults |
| `Clock` | Running from construction; accumulated active time `Duration::ZERO` |
| `WindowBuilder` | title `VMNL Window`, `800x600`, common event delivery enabled, joystick event delivery/tracking disabled, zero-dead-zone `Math2D` left/right sticks, built-in 2D shaders, opaque black clear, no size limits, automatic present mode |
| `PresentMode` | `Auto`: `Mailbox` → `Immediate` → `FifoRelaxed` → `Fifo` among supported modes |
| Resolved window attachments | one color attachment, one sample per pixel; no MSAA selection |
| `SurfaceColorConfig` | `legacy_auto`: prefer `B8G8R8A8_SRGB / SRGB_NON_LINEAR`, then the first recognized pair with `SRGB_NON_LINEAR`, then the first recognized pair with an enabled color space. `prefer` skips formats without blending support and defaults to `Reject` if no pair qualifies |
| `RenderTargetConfig` / `WindowBuilder::render_target` | color only; no depth image; clear depth `1.0` |
| `RenderMode` | `PerObject`; `Batched` currently falls back to it |
| `Input`, `KeyboardState`, `MouseState` | Keyboard and mouse states inactive; each joystick slot is `NotTracked` |
| Window sticky keys / sticky mouse buttons / lock-key modifier reporting | Disabled |
| `CursorMode` / new window cursor | `Normal`; backend default cursor; raw mouse motion disabled |
| `CursorBuilder` | hotspot `(0, 0)`; diagnostic hotspot marker disabled |
| `Modifiers` | `NONE` |
| `Anchor` | `TopLeft` |
| `LineCap` | `Butt` |
| `LineJoin` | `Bevel` |
| Polyline | open; width `1.0`; butt cap; bevel join; miter limit `4.0`; opaque white; device memory preference |
| Polygon | ordered simple convex or concave boundary; white from points, supplied colors from vertices; device memory preference; implicit closure; convex triangle fan or deterministic concave ear clipping |
| Rectangle | position `(0,0)`, white, rotation `0°`, top-left origin, sharp corners, `8` corner segments, device memory preference |
| Circle / ellipse through `Shape::circle` | equal required radii until `.ellipse(ry)`; center `(0,0)`, unfilled, white, 32 segments, inner/outer offsets `0.5`, no sector or cap, device memory preference; outer offset applies even when filled |
| Triangle | white per vertex unless created from colored vertices; device memory preference |
| Line | width `1.0`, butt cap, white, device memory preference |
| Indexed shape / raw geometry / uniform | device memory preference |
| `PipelineSpec` | shaders missing, `TriangleList`, blend inherited from the context then the opaque preset, `CullMode::None`, `FrontFace::CounterClockwise`, `PolygonMode::Fill`, line width `1.0`, viewport/scissor `FullFramebuffer`, depth `Disabled` |
| `ViewportPolicy` / `ScissorPolicy` | `FullFramebuffer`, resolved against the acquired image extent on each raw draw |
| `GeometryBuilder` | no indices |
| `PixelSource` | tightly packed top-left rows; encoding follows `TextureFormat`; straight alpha; automatic usage; no explicit range, so the supplied length must equal the required end |
| automatic texture usage | sampled plus transfer destination; no implicit transfer source |
| `Texture2DBuilder` / image allocation | optimal tiling; one mip level; device-preferred image memory; `General` layout; `Wait` retirement for pending upload |
| `Texture2DBuilder::with_mutable_format_views` | inherits the context policy or disables mutable-format views |
| `TextureView2DBuilder` | inherits context fields; otherwise the storage format and identity mapping, with texture declarations inherited only by an unchanged view |
| `PngDecodeOptions::decode_png` | untagged input defaults to sRGB; PNG alpha or `tRNS` defaults to straight alpha; otherwise opaque alpha |
| `SamplerConfig` / `.linear()` | linear min/mag/mipmap filters; U/V clamp-to-edge; transparent float border; min LOD 0; unbounded max LOD; bias 0; anisotropy 1 (disabled); normalized coordinates |
| `SamplerConfig::nearest()` | nearest min/mag/mipmap filters; all other settings match the linear preset |
