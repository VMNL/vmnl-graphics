# VMNL Examples

Example policy lives in [docs/examples.md](../docs/examples.md).

Run examples from the repository root:

```bash
just run <example>
just build <example>
```

| Example | Command | Covers |
| --- | --- | --- |
| [`d2_alpha_order`](d2/alpha_order/src/main.rs) | `just run d2_alpha_order` | Start with blue drawn over red. Press Space to reverse the two translucent passes. Only the overlap changes color. |
| [`d2_arc`](d2/arc/src/main.rs) | `just run d2_arc` | Two equal elliptical arcs. The left has flat ends and the right has round ends. |
| [`d2_circle`](d2/circle/src/main.rs) | `just run d2_circle` | Both setter orders give identical cyan discs in the top row and identical yellow outlines in the lower row. |
| [`d2_ellipse`](d2/ellipse/src/main.rs) | `just run d2_ellipse` | Compare the filled cyan ellipse on the left with the outlined yellow ellipse on the right. |
| [`d2_indexed`](d2/indexed/src/main.rs) | `just run d2_indexed` | Four indexed vertices form a quad with color interpolation across its two triangles. |
| [`d2_line`](d2/line/src/main.rs) | `just run d2_line` | Three equal-width lines show flat, round and square caps from top to bottom. |
| [`d2_polygon`](d2/polygon/src/main.rs) | `just run d2_polygon` | A convex pentagon and a concave polygon show vertex colors. The right polygon keeps its inward notch. |
| [`d2_polyline`](d2/polyline/src/main.rs) | `just run d2_polyline` | Compare bevel, miter and round joins from left to right, with segment, point and uniform colors. |
| [`d2_polyline_joins`](d2/polyline_joins/src/main.rs) | `just run d2_polyline_joins` | Compare bevel (left), miter (center) and round (right) using identical points relative to each panel, width, caps and color. |
| [`d2_rectangle`](d2/rectangle/src/main.rs) | `just run d2_rectangle` | A solid cyan rectangle appears in the center of the window. |
| [`d2_rectangle_transform`](d2/rectangle_transform/src/main.rs) | `just run d2_rectangle_transform` | See the center anchor on the left and an off-center origin on the right. White markers identify the pivots. |
| [`d2_ring`](d2/ring/src/main.rs) | `just run d2_ring` | Two complete rings show a width on the left and an explicit inner offset on the right. |
| [`d2_rounded_rectangle`](d2/rounded_rectangle/src/main.rs) | `just run d2_rounded_rectangle` | Compare uniform, asymmetric, rotated and partially rounded corners. |
| [`d2_sector`](d2/sector/src/main.rs) | `just run d2_sector` | A green filled quarter-circle appears, with two straight radial edges. |
| [`d2_triangle`](d2/triangle/src/main.rs) | `just run d2_triangle` | Equivalent triangles show the same three-color gradient through the two triangle constructors. |
| [`input_cursor`](input/cursor/src/main.rs) | `just run input_cursor` | Press S for the standard pointing hand, U for a custom cross and O for the backend default. |
| [`input_cursor_modes`](input/cursor_modes/src/main.rs) | `just run input_cursor_modes` | Move the mouse and press N/V/D/G to change modes. Press P to center it on supported backends. In Disabled mode, M toggles raw motion where supported. |
| [`input_joystick`](input/joystick/src/main.rs) | `just run input_joystick` | Connect a joystick in slot 1 to see a connection event. With a mapped gamepad, press and release A to observe button state and the sampled stick. |
| [`input_keyboard`](input/keyboard/src/main.rs) | `just run input_keyboard` | Press, hold and release a physical key. Verify event metadata, repeat flags and down/pressed/released state. F2 toggles sticky keys. |
| [`input_mouse`](input/mouse/src/main.rs) | `just run input_mouse` | Move, click and scroll inside the window. Check cursor coordinates, button transitions and the latest scroll deltas. |
| [`input_text`](input/text/src/main.rs) | `just run input_text` | Type text and check Unicode characters in the output. F9 toggles text and F10 toggles legacy modified text on supported backends. |
| [`joystick_runtime`](input/joystick_runtime/src/main.rs) | `just run joystick_runtime` | On a usable GLFW backend with a controller, print one standalone joystick/gamepad sample. |
| [`raw_blend`](raw/blend/src/main.rs) | `just run raw_blend` | Compare an opaque triangle on the left with the matching straight-alpha and premultiplied triangles in the center and on the right. |
| [`raw_culling`](raw/culling/src/main.rs) | `just run raw_culling` | C cycles cull modes for opposite-winding triangles. In Front or Back mode, F swaps the visible triangle. |
| [`raw_d2_composition`](raw/d2_composition/src/main.rs) | `just run raw_d2_composition` | The raw quad covers part of the 2D background. The white 2D marker appears in front of both. |
| [`raw_depth`](raw/depth/src/main.rs) | `just run raw_depth` | Near yellow and far purple triangles overlap. D cycles depth states and O reverses draw order. Yellow wins either order with Less and writes. Select d16 or d32 with `VMNL_DEPTH_FORMAT`. |
| [`raw_frame_uniform`](raw/frame_uniform/src/main.rs) | `just run raw_frame_uniform` | A triangle moves and changes tint. Space pauses/resumes while the frame uniform is still written on every submission. |
| [`raw_indexed_geometry`](raw/indexed_geometry/src/main.rs) | `just run raw_indexed_geometry` | Four raw vertices form two indexed triangles that fill a quad. |
| [`raw_line_width`](raw/line_width/src/main.rs) | `just run raw_line_width` | With `WideLines` and a supported width above one, the right line is wider than the unit-width left line. |
| [`raw_point_size`](raw/point_size/src/main.rs) | `just run raw_point_size` | With `LargePoints`, three clearly enlarged color points appear. |
| [`raw_scissor`](raw/scissor/src/main.rs) | `just run raw_scissor` | Space toggles a fixed framebuffer clip rectangle. Only pixels inside the rectangle remain, with the same triangle transform. |
| [`raw_shader_path`](raw/shader_path/src/main.rs) | `just run raw_shader_path` | The raw pipeline loads local vertex and fragment files and draws a colored triangle. |
| [`raw_texture_address`](raw/texture_address/src/main.rs) | `just run raw_texture_address` | Press Space to cycle edge clamp, repeat, mirror and black border. Only the texture sample behavior outside U = 0..1 changes. |
| [`raw_texture_bindings`](raw/texture_bindings/src/main.rs) | `just run raw_texture_bindings` | The combined sampler on the left and separate image/sampler on the right draw the same four texels. |
| [`raw_texture_filter`](raw/texture_filter/src/main.rs) | `just run raw_texture_filter` | Nearest on the left produces hard texel boundaries. Linear on the right blends across the same boundaries. |
| [`raw_texture_png`](raw/texture_png/src/main.rs) | `just run raw_texture_png` | The local PNG appears as red/green texels above blue/white texels. |
| [`raw_texture_view`](raw/texture_view/src/main.rs) | `just run raw_texture_view` | Red and blue swap on the right compared with the identity view. Green and white stay the same. |
| [`raw_topology`](raw/topology/src/main.rs) | `just run raw_topology` | Press Space to cycle triangles, lines and points assembled from the same six vertices. |
| [`raw_triangle`](raw/triangle/src/main.rs) | `just run raw_triangle` | A minimal raw pipeline draws one triangle with three vertex colors. |
| [`raw_uniform`](raw/uniform/src/main.rs) | `just run raw_uniform` | The uniform written before the first draw supplies a fixed tint and shifts the triangle right. |
| [`raw_viewport`](raw/viewport/src/main.rs) | `just run raw_viewport` | Space toggles between full and fixed viewport transforms. Resize and verify the fixed viewport keeps its pixel dimensions. |
| [`raw_viewport_scissor`](raw/viewport_scissor/src/main.rs) | `just run raw_viewport_scissor` | V and S toggle fixed viewport/scissor. Resize across the four states and verify the later pass markers stay unaffected. D16/D32 enable the depth interaction. See the [full procedure](../docs/api/workflows/compose_2d_and_raw.md#inspect-viewport-and-scissor-policies). |
| [`raw_wireframe`](raw/wireframe/src/main.rs) | `just run raw_wireframe` | With `FillModeNonSolid`, Space toggles the filled quad and its wireframe, including the shared diagonal. |
| [`window_clock`](window/clock/src/main.rs) | `just run window_clock` | Start with two clocks running. Press Space to pause A while B continues. Press Space again to resume A or R to restart it. |
| [`window_config`](window/config/src/main.rs) | `just run window_config` | Resize to check the requested 16:9 ratio where supported. Press Space to clear it and resize freely. |
| [`window_custom_shaders`](window/custom_shaders/src/main.rs) | `just run window_custom_shaders` | See two shapes using the file-backed 2D shaders. Use `VMNL_INLINE_SHADERS=1` for the same output from inline source. |
| [`window_event_polling`](window/event_polling/src/main.rs) | `just run window_event_polling` | Space toggles key event delivery. Press and release A to verify the snapshot still changes. File drop and scale events are also reported. |
| [`window_lifecycle`](window/lifecycle/src/main.rs) | `just run window_lifecycle` | Use F/I/M/R/H for focus, minimize, maximize, restore and hide/show. Observe the resolved window state where the backend supports the request. |
| [`window_monitors`](window/monitors/src/main.rs) | `just run window_monitors` | Check the listed monitor names, primary display and work area. Press Space to query again. |
| [`window_surface_color`](window/surface_color/src/main.rs) | `just run window_surface_color` | The title names the selected pair and reason. Resize and check that the translucent rectangle keeps rendering and the title keeps the same pair. `VMNL_SURFACE_COLOR_UNORM=1 just run window_surface_color` requests BGRA8 UNORM with nonlinear sRGB exactly, and requires that pair on the surface. |
| [`window_timer`](window/timer/src/main.rs) | `just run window_timer` | Press Space twice with a delay. Each press prints elapsed seconds and high-resolution ticks, then resets elapsed time. |
| [`window_wait_events`](window/wait_events/src/main.rs) | `just run window_wait_events` | The first wait is explicitly woken. Press Space to change the rectangle color and resize to verify redraw after a transient out-of-date result. |
