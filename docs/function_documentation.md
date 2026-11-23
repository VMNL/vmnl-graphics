# Function and Method Documentation

Choose the format by the caller and the contract they need. The public formats cover functions and
methods callable by clients, including re-exports, methods on public VMNL traits, and methods on
builders returned by public APIs. Apply the internal formats to backend implementation code,
including the derive support reserved under `raw::__private`. See the [API boundaries](architecture.md#domain-scope).

Write comments in English. Start each `///` comment with a short summary sentence in the third
person. Include applicable sections in the order shown below. Treat a library panic as a defect
under the [library error policy](RUST.md#error-management).

## High-level public API

Describe the operation in application terms. Explain observable results, defaults, and state
changes that affect the caller. Describe the effect on `self` in the contract when the call
consumes it or changes its state.

```text
/// [Summarizes the operation from the client's perspective.]
///
/// [Explains the result, defaults, and changes to application state.]
///
/// # Arguments
///
/// - `name` [Role, where to obtain it, and applicable units or valid values.]
///
/// # Errors
///
/// [Returned variants, their triggers, and relevant state after failure.]
///
/// # Example
///
/// [Shows the call in a typical client workflow.]
```

## Low-level public API

Apply this format to public low-level entry points such as those in `raw`. Describe the compatibility
of inputs and resources, their ownership and lifecycle, and the costs and synchronization visible
to the caller. Identify platform, feature, or device requirements where applicable. For generic
inputs, explain how clients can satisfy bounds such as `BufferContents`.

```text
/// [Summarizes the resource or rendering operation.]
///
/// [Explains device ownership, resource state, and relevant allocation,
/// transfer, submission, or synchronization behavior.]
///
/// # Arguments
///
/// - `name` [Role, how to obtain or construct it, and compatibility constraints.]
///
/// # Errors
///
/// [Returned variants, the stage that fails, and what can be reused or retried.]
///
/// # Example
///
/// [Shows how to obtain compatible inputs and use the result.]
```

For both public formats, cover each explicit parameter under `# Arguments`. Use `# Errors` for
errors returned by this call. Explain ordinary outcomes such as `None` or a timeout returned in
`Ok` in the contract. Document the shared contract on a public trait method and any
implementation-specific behavior where callers of that implementation discover it.

Write examples against public import paths and show where important inputs come from. Use doctests
that run for deterministic headless examples and `rust,no_run` when execution requires a GPU or a
window. Follow the [completion checks](../CONTRIBUTING.md#before-submission) when Rustdoc changes.

When interpreting a return value requires a choice, show how the caller uses it in a branch,
validation, or subsequent call. For example, match a pipeline's depth policy before using it for
a pass that requires depth writes. Use a focused example with an existing resource as an input
when the decision is easier to explain on its own.

A shared example can replace a local snippet when it shows the exact call and any meaningful use
of the result. Link directly to that example block. For a simple accessor, the summary can
suffice when the type's example already demonstrates the call.

### Choosing and constructing arguments

Document shared choices on the public type and link to it from `# Arguments`. Put details that
affect a call before `# Example`. Explain a rule that spans several arguments or builder settings
in the method contract. Use a short list when callers need to compare several applicable choices.

| Case | Information the caller needs |
| --- | --- |
| Enum variants | Describe each variant and its fields on the enum. At the call site, identify the default, accepted variants, or rules that change the meaning of a variant for this call. Link to individual variants when explaining those rules. |
| Omitted settings and special values | Give the effect of omission, `None`, or any sentinel value. Distinguish inherited settings, explicit values, and resets when the API supports them. |
| Combinable flags | Describe each flag on its type. At the call site, explain how to combine flags and specify any required, incompatible, or implied flags. |
| Related arguments and settings | Describe dependencies such as matching lengths, required pairs, and settings that replace earlier choices. Specify which operation checks the combination. |
| Generic inputs and conversions | Show common accepted forms and explain what a conversion supplies, such as an implicit alpha value, or what the caller must ensure, such as a shader-compatible layout. |
| Resource provenance | Name the constructor or source and the context, device, window, or pipeline that the argument must belong to. Link to a public compatibility check when one exists. |
| Callbacks | Explain when and on which thread the callback runs, what its arguments borrow, and whether it can re-enter the API or replace the registration. |

For deferred validation, name the validating operation next to the configuration rule and link to
that operation. Put the returned error variants and their triggers in its `# Errors` section.

## Shared backend

Use `///` for internal callable contracts shared across modules. Describe what the caller must
establish, what the helper guarantees, and any ordering, lifetime, or ownership invariant the
caller relies on.

```text
/// [Summarizes the internal operation and its result.]
///
/// [States the caller's preconditions and the guarantees after the call.]
///
/// # Errors
///
/// [Distinguishes failures the caller must handle and any partial effects.]
```

Use `# Arguments` before `# Errors` when individual parameters need separate explanations. For
example, [`preflight_indexed_geometry`](../crates/vmnl_graphics/src/2d/shape/indexed.rs) checks
triangle structure, bounds, and counts before checking positions. Callers complete this validation
before allocating either GPU buffer.

## Local backend helpers

Place `//` next to a decision that needs its rationale or invariant explained. A helper with an
obvious contract can rely on its name and signature.

```text
// [Explains why this ordering, guard, or resource lifetime is required here.]
```

## Tests, examples, and tooling

- In tests, explain why a fixture or sequence of calls exercises an invariant when the code leaves
  that reason implicit.
- In examples, explain why a step is required in the workflow being demonstrated.
- In tooling, document which files or external state are read or changed when callers need that
  information. Use `///` for a shared Rust tool entry point whose inputs, effects, or failures need
  to be documented. Follow the comment convention of the tool's language.

## Unsafe and FFI

For every `unsafe` function, use `# Safety` to state the invariants the caller must uphold. Place it
after `# Errors` and before `# Example` when those sections apply. At each `unsafe` block, use an
adjacent `// SAFETY:` comment to explain how the invariants are satisfied at that point. State the
relevant source of the guarantee, such as a type, a validation step, the handle owner, a thread
restriction, or a synchronization guard.
