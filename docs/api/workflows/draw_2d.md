# Draw in 2D

1. Build shapes once from the same `Context` used by the window.
2. Start a frame with `window.render()`.
3. Append one or more `draw2d` passes; call order is preserved.
4. Call `submit` and handle resize/out-of-date errors according to the application loop.

Build a connected stroke with [`Shape::polyline`](../reference/d2/shapes/shape.md) and configure its caps, joins, and color mode with [`PolylineBuilder`](../reference/d2/shapes/polyline_builder.md). The [polyline example](../../../examples/d2/polyline/src/main.rs) shows joins, caps and color modes. [`d2_polyline_joins`](../../../examples/d2/polyline_joins/src/main.rs) compares bevel, miter and round joins with identical stroke settings.

Use [`Shape::circle`](../reference/d2/shapes/circle_builder.md) for the circle family:
`.ellipse(ry)` changes the vertical radius, `.width(w)` sets the stroke width, and
`.sector(start, sweep).cap(LineCap::...)` selects an angular interval. Sector angles are degrees,
zero at the top, positive clockwise. Every sector requires a cap, including filled sectors.
Width setters preserve filling. An explicit `.filled(true)` keeps the outer boundary offset.
Run [`d2_circle`](../../../examples/d2/circle/src/main.rs) for the fill/width comparison. The
[example catalogue](../../../examples/README.md) lists independent ellipse, ring, sector and arc scenarios.

```rust,no_run
# extern crate vmnl_graphics;
use vmnl_graphics::{Context, Window};
use vmnl_graphics::common::Rgba;
use vmnl_graphics::d2::Shape;

fn main() -> vmnl_graphics::VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::new(&context)?;
    let rect = Shape::rect(200.0, 100.0).color(Rgba::RED).build(&context)?;
    window.render().draw2d([&rect]).submit()?;
    Ok(())
}
```

Start with [`d2_rectangle`](../../../examples/d2/rectangle/src/main.rs). In [`d2_alpha_order`](../../../examples/d2/alpha_order/src/main.rs), press Space to reverse the order of two translucent passes and compare the overlap. See [`Shape`](../reference/d2/shapes/shape.md) and [`FrameRenderer`](../reference/window/rendering/frame_renderer.md).

Use [`Shape::polygon`](../reference/d2/shapes/polygon_builder.md) for an ordered
simple convex or concave boundary without supplying indices. [`d2_polygon`](../../../examples/d2/polygon/src/main.rs) demonstrates per-vertex colors.
Use [`d2_indexed`](../../../examples/d2/indexed/src/main.rs) to control triangulation explicitly.
