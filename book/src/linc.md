# LINC Reference

LINC is the link-surface, symbol-inventory, validation, and ABI-evidence layer in the `PARC → LINC → GERC` pipeline.

Its job is not to parse C source and not to generate Rust code.
Its job is to take normalized source contracts and native artifacts, then produce link and binary evidence that downstream tooling can trust.

In practice LINC sits between:

- `parc`, which handles preprocessing, parsing, and declaration extraction
- native artifacts such as `.o`, `.a`, `.so`, and `.dylib`
- downstream consumers such as `gec` (Rust projection), `fol`, or validation/reporting tooling

## What LINC Produces

The core output is a `LinkAnalysisPackage`.

That package is intentionally narrower than the historical all-in-one IR. It contains:

- target/compiler metadata for the analysis
- declared and normalized native link inputs
- diagnostics produced during analysis
- optional resolved link-plan data
- optional probe and validation attachment points

When native artifacts are involved, LINC can also produce:

- `SymbolInventory` values from `inspect_symbols`
- `ValidationReport` values from `validate`
- `ResolvedLinkPlan` values from `resolve_link_plan`

## Data flow

```text
PARC (parc)
    -> SourcePackage (frontend-neutral contract)
    -> LINC (linc)
    -> LinkAnalysisPackage / link and binary evidence
    -> GERC (gec)
    -> Rust projection / emitted crate
```

## What LINC Owns

- intake of normalized frontend/source contracts
- binary symbol inspection
- object/shared-library/archive metadata extraction
- provider matching
- link-plan construction
- ABI probe orchestration and retained measurement evidence
- declaration-vs-artifact validation
- link and binary evidence reporting

## What LINC Does Not Own

- source parsing or preprocessing (upstream: `parc`)
- source-level declaration extraction (upstream: `parc`)
- Rust FFI code generation (downstream: `gec`)

## Module and API surface

Most users touch one or more of these library entry points:

- `analyze_source_package` for ingesting a `SourcePackage` from any frontend
- `probe_type_layouts` for compiler-assisted ABI layout probing
- `inspect_symbols` for reading native artifact symbols
- `validate` and `validate_many` for declaration-vs-artifact checks
- `resolve_link_plan` for link-plan construction
- `serde_json` over the final explicit contracts when transport is needed

## Artifact boundary

`linc` owns evidence, not universal pipeline state.

The boundary rule is:

- `linc/src/**` must not depend on `parc` or `gec`
- cross-package translation belongs only in tests, examples, or external harnesses
- repo-local bootstrap utilities are secondary, not the public architecture

## Recommended Reading Order

1. Getting Started and the core extraction chapters
2. Native Evidence
3. API Contract and the contract/policy chapters
4. End-To-End Workflows
5. Operations And Release

If you only want to integrate LINC into another tool, focus on:

- [Header Processing](./020_headers.md)
- [IR Model](./030_ir.md)
- [Native Evidence](./095_native_evidence.md)
- [API Contract](./100_api_contract.md)
- [End-To-End Workflows](./110_workflows.md)
