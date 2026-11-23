# Examples

The [example catalogue](../examples/README.md) lists every runnable example in the workspace
and its manual scenario. Examples are public-API client programs for visual or interactive
workflows. The windowless `joystick_runtime` example also shows standalone `InputRuntime`
access. It initializes GLFW and requires a usable backend.

## Decide when to create an example

- Create a focused example for a new shape or a public behavior that needs its own scene,
  action, or platform/GPU prerequisite. Extract a scenario from an existing example when
  its independent prerequisites or controls make a focused check harder to run.
- Extend an example when the variants verify the same observable contract. For example,
  comparing two setter orders for circle fill and width belongs in one program.
- Keep an integration example when the interaction itself is the contract, such as viewport
  and scissor state isolation across draws after a resize.
- Put deterministic assertions in the appropriate [test suite](testing.md#test-types). A
  visible correction can reuse an example that already exercises the defect. Create a
  focused example when reproducing it requires a different visual scene.

## Write the scenario

1. Give the package one scope and a matching name. For example,
   `examples/d2/rounded_rectangle` is built with `just build d2_rounded_rectangle` and run
   with `just run d2_rounded_rectangle`.
2. Document the initial state, operator action and expected observation in the catalogue.
   Link to an API workflow for longer procedures. Give exact options, keys and prerequisites
   where they matter. Identify physical keys where layout can change the key label.
3. Keep the client loop and public VMNL calls visible in the source. Use stable initial
   geometry and vary one setting at a time. Show the active setting in the title or output
   and report resolved dimensions, formats or capabilities when they affect the result.
4. Keep assets with their example and resolve runtime paths relative to
   `CARGO_MANIFEST_DIR`. Request only the device features exercised by the scenario.

For a graphics PR, check the changed scenario and relevant interactions. Record the command,
actions and actual operator observations in `Validation` according to
[CONTRIBUTING.md](../CONTRIBUTING.md#pull-request-descriptions).

## Scaffold and register an example

```bash
just example-new d2 outline_demo --dry-run
just example-new d2 outline_demo
cargo update --workspace
just example-check
just build d2_outline_demo
```

`example-new` creates a runnable starter in `examples/<domain>/<name>`, registers the
workspace member, and adds a catalogue row. The supported domains are `d2`, `raw`,
`window`, and `input`. Use `--author "Name"` to override the Git author for the SPDX header.
Cargo updates the shared lockfile separately. The check verifies that Cargo can select the
linked binary and that the package and version are present in the lockfile.
If a concurrent edit prevents a clean rollback, the generator preserves that edit and reports
which files and scaffold need inspection before retrying.
Replace the starter scene and catalogue entry
with the intended scenario. When migrating an example, move its documented observations
and update incoming links, example commands, and defaults in the same change.
