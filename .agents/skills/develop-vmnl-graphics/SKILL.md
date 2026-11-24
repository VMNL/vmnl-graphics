---
name: develop-vmnl-graphics
description: Develop, fix, review, document, and validate VMNL graphics work across the public vmnl_graphics API, related macros, Vulkan, windowing, input, rendering, GPU resources, tests, examples, and graphics tooling. Use for any task affecting VMNL graphics architecture, public API, GPU behavior, graphics correctness, tests, examples, or performance, and when asked to draft a VMNL Graphics pull request description. Excludes unrelated audio and networking work.
---

# Develop VMNL Graphics

## Workflow

1. Follow the root `AGENTS.md`; classify the task and its impacts before editing.
2. Read only the canonical documentation and agent references routed below.
3. Inspect the affected contract, tests, and minimal implementation path.
4. Implement the smallest coherent patch while preserving the progressive control model, predictable costs, scoped determinism, ownership, and layer separation in `docs/architecture.md`.
5. Run a targeted check during development, then the applicable completion and graphics-specific checks.
6. Report only directly observed automatic evidence and operator-reported manual evidence.

## Route Canonical Documentation

| Impact | Read |
| --- | --- |
| Architecture, builders, public API | [`docs/architecture.md`](../../../docs/architecture.md), [`docs/API.md`](../../../docs/API.md) |
| Function and method Rustdoc or comments | [`docs/function_documentation.md`](../../../docs/function_documentation.md) |
| Build, dependencies, shaderc | [`docs/build.md`](../../../docs/build.md), [`docs/troubleshooting.md`](../../../docs/troubleshooting.md) |
| Tests | [`docs/testing.md`](../../../docs/testing.md) |
| Examples | [`docs/examples.md`](../../../docs/examples.md), [`examples/README.md`](../../../examples/README.md) |
| Platform-sensitive behavior | [`docs/platform_support.md`](../../../docs/platform_support.md) |
| Contribution or pull request | [`CONTRIBUTING.md`](../../../CONTRIBUTING.md) |

Treat `docs/` as the canonical home for stable architecture, contracts, and procedures. This skill contains operational guidance only; do not copy canonical documentation into it.

## Draft Pull Request Descriptions

When the user requests a PR description, read the
[shared writing guide](../../references/github-writing.md) and follow the exact format and
evidence rules in [CONTRIBUTING.md](../../../CONTRIBUTING.md#pull-request-descriptions).

Confirm the base and head or the requested worktree scope. Read the diff and the relevant files or
contracts. In `Context`, explain the problem and the intended result. In `Changes`, group related
edits by what they accomplish. Explain important decisions and their consequences for callers,
architecture, performance, or compatibility where relevant. Include material risks or limitations
that remain. Summarize what the changed tests and documentation cover. Name files when they help
the reviewer find an important change.

Check which revision and scenario each reported result covers. Automated results belong in the PR
checks or CI. Follow the operator evidence rule for `Validation` and flag missing required evidence
outside the draft.

## Route Agent References

- Read [`../../references/module-design.md`](../../references/module-design.md) for a module
  interface, crate boundary, raw API, backend adapter, or FFI seam decision.
- Read [`../../references/diagnostic-protocol.md`](../../references/diagnostic-protocol.md) for a
  bug, regression, incorrect result, crash, hang, or performance diagnosis.
- Read [`references/current-limitations.md`](references/current-limitations.md) before changing context/device selection, GLFW initialization or GPU test routing.
- Read [`references/graphics-correctness.md`](references/graphics-correctness.md) for Vulkan, synchronization, swapchain, window, resource, shader, pipeline, unsafe, platform, or performance work.
- Read [`references/validation.md`](references/validation.md) for every feature, fix, Rust source change, test/example change, tooling change, dependency change, or validation claim.

## Choose the Vulkan Implementation Path

Use Vulkano where it satisfies the established contract. For any graphics task, direct Vulkan
calls, including through `ash`, are allowed for an operation when the relevant high-level and
lower-level Vulkano APIs cannot meet its contract under the repository rules. Keep the native
path private and follow the [native Vulkan interoperability rules](references/graphics-correctness.md#native-vulkan-interoperability).

## Preserve Graphics Contracts

- For API design and audits, apply the [explicit client control contract](../../../docs/architecture.md#explicit-client-control)
  and the policy inventory in [module design](../../references/module-design.md#evaluate-the-interface)
  before accepting the proposed surface as complete.
- Keep explicit low-level pipeline and geometry control in `raw`.
- Keep every observable high-level default inspectable and replaceable without requiring the raw API.
- Do not expose backend types as stable public contracts without an explicit raw-API decision.
- For a public graphics change, decide public crate exposure and document applicable units, coordinates, defaults, valid ranges, ownership, lifecycle, costs, synchronization, errors, safety requirements, and platform/GPU requirements.
- Verify claims such as allocation-free, zero-copy, asynchronous, non-blocking, deterministic, or constant-time before documenting them.
