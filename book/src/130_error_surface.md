# Error Surface

This chapter inventories the current public error surface of LINC.

It is intentionally explicit because one of the main goals of the new roadmap is to replace the remaining unstructured operational errors with typed crate errors.

## Current State

LINC currently has two different public error styles:

- typed errors via `LincError` (also available as the `LincError` alias)
- unstructured `Result<_, String>` returns on several operational APIs

That split is functional, but it is not yet the final intended API shape.

## Typed Error Surface Today

The clearest typed error boundary today is around JSON transport:

- `to_json(...) -> Result<String, LincError>`
- `from_json(...) -> Result<BindingPackage, LincError>`

This is currently the most structured part of the error model.

## Remaining `String`-Returning APIs

The current APIs still returning `Result<_, String>` are:

| API | Area | Why it currently uses `String` |
|---|---|---|
| `HeaderConfig::process` | raw-header scan | preprocessing, parsing, probe, and scan orchestration errors are not yet normalized |

Note: `extract_from_source` has been narrowed to `pub(crate)` and is no longer part of the public API surface. New consumers should use `from_source_package` (intake) instead.

These are precisely the APIs targeted by the next error-model workstream.

## What Consumers Should Do Right Now

Until typed operational errors land, downstream users should:

- treat successful return values as stable enough to consume
- treat diagnostics in returned data structures as first-class signals
- avoid matching exact error strings for durable control flow
- wrap `String` errors at their own boundary if they need structured handling immediately

## What Counts As An Error vs A Diagnostic

This distinction is not fully standardized yet, but the current pattern is:

- hard operational failures generally return an error
- partially understood source constructs often become diagnostics attached to a returned package
- validation findings are reported as structured match results, not thrown as errors

That is already a useful separation.
The remaining work is to make the hard-failure side typed and explicit.

## Why This Inventory Matters

Without an explicit inventory, it is easy for stringly APIs to linger indefinitely.

This chapter exists to make the current debt auditable.
The next steps are not "discover the problem later."
They are:

1. define the typed taxonomy
2. migrate these APIs one by one
3. add tests that treat error categories as stable contract
