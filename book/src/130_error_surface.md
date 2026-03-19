# Error Surface

This chapter inventories the current public error surface of `bic`.

It is intentionally explicit because one of the main goals of the new roadmap is to replace the remaining unstructured operational errors with typed crate errors.

## Current State

`bic` currently has two different public error styles:

- typed errors via `BicError`
- unstructured `Result<_, String>` returns on several operational APIs

That split is functional, but it is not yet the final intended API shape.

## Typed Error Surface Today

The clearest typed error boundary today is around JSON transport:

- `to_json(...) -> Result<String, BicError>`
- `from_json(...) -> Result<BindingPackage, BicError>`

This is currently the most structured part of the error model.

## Remaining Public `String`-Returning APIs

The current public APIs still returning `Result<_, String>` are:

| API | Area | Why it currently uses `String` |
|---|---|---|
| `extract_from_source` | direct extraction | parser/extractor plumbing still uses stringly operational failures |
| `HeaderConfig::process` | raw-header scan | preprocessing, parsing, probe, and scan orchestration errors are not yet normalized |
| `inspect_symbols` | artifact inspection | file/format/tooling failures are not yet modeled with a typed taxonomy |

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
