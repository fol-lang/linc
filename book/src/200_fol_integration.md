# `fol` Integration Guide

This chapter describes the intended producer/consumer contract between LINC and `fol`.

LINC should be read as the C-analysis producer.
`fol` should be read as the downstream consumer that decides what to generate, what to reject, and
how to assemble its final binding/link workflow.

## Division Of Responsibility

The intended split is:

- LINC extracts declarations, metadata, diagnostics, layouts, and native-surface evidence
- `fol` consumes that evidence to generate bindings, choose generation policy, and drive any final
  language/runtime-specific integration

LINC is not expected to replace `fol`.
It is expected to provide a stable machine contract that `fol` can trust.

## What LINC Should Produce For `fol`

For a healthy integration, `fol` should expect LINC to provide:

- `BindingPackage` as the primary declaration and metadata contract
- `BindingPackage.diagnostics` as explicit extraction warnings and partial-fidelity signals
- `BindingPackage.layouts` when ABI-sensitive types need compiler-probed evidence
- `BindingPackage.link` as the normalized native dependency surface
- `SymbolInventory` and `ValidationReport` when native artifact matching matters

## What `fol` Should Treat As Required Inputs

For simple declaration-only generation, `fol` can often start with:

- `BindingPackage.items`
- `BindingPackage.diagnostics`

For ABI-sensitive or publication-quality generation, `fol` should generally also require:

- expected layout evidence in `BindingPackage.layouts`
- link metadata in `BindingPackage.link`
- validation evidence against real native artifacts where applicable

## Recommended End-To-End Flow

The intended high-confidence flow is:

1. run LINC header scanning or preprocessed parsing
2. inspect `BindingPackage.diagnostics`
3. require layout probes for ABI-sensitive types
4. inspect native artifacts and run validation when linkable symbols matter
5. serialize or pass the resulting structured values to `fol`
6. let `fol` decide what to generate, reject, or gate behind policy

When `fol` applies probe-aware gating, it should read retained probe diagnostics intentionally:

- `ProbeUnavailable` means the requested type did not produce honest layout evidence and should
  usually be treated as "do not lower by value unless you have another trusted reason"
- `ProbeFailed` means the probe mechanism itself was not trustworthy for that request and should
  normally block generation until fixed

That distinction matters because an opaque-handle API and a broken probe request are not the same
problem, even though both can leave `BindingPackage.layouts` without the requested subject.

## Contract Boundaries

The key contract boundaries are:

- `schema_version` is the wire-compatibility gate
- `linc_version` is producer provenance
- `BindingPackage` is the declaration/metadata contract
- `ValidationReport` is evidence, not an exception channel
- diagnostics are part of the data contract, not incidental logs

## Fixture-Driven Consumer Testing

This repository also carries serialized fixture packages for downstream-consumer tests.

Those fixtures are example consumer fixtures, not the universal crate contract.
They exist so a downstream tool such as `fol` can pin a narrow, intentional subset of the generic
library contract and regression-test it without depending on live repository state.

The current fixture tiers are:

- a minimal package fixture that exercises the smallest durable generation contract
- an extended package fixture that exercises additive evidence such as macros, layouts, and link
  metadata

The repository now also carries producer-side `fol` acceptance tests that exercise:

- a live binding-scan flow serialized into a narrow consumer subset
- a native validation plus link-plan flow serialized into a narrow consumer subset

That keeps the generic library contract primary while still regression-testing the concrete `fol`
integration profile.

## What `fol` Should Not Assume

`fol` should not assume:

- every successful scan is generation-ready
- declarations alone are enough for ABI-sensitive output
- every platform has equal artifact-inspection maturity
- exact free-form error strings are stable control-flow keys
- missing validation means "safe", rather than "not yet checked"

## Recommended Consumer Gates In `fol`

`fol` should make these gates explicit in its own pipeline:

- block on operational errors
- block on diagnostics that affect required declarations
- block on missing required layouts
- block on validation findings for required native symbols
- block on unsupported `schema_version`

These gates should be policy decisions in `fol`, but the evidence should come from LINC.

## Recommended Gating Policy

If `fol` wants one concrete, conservative policy that matches the current producer-side
acceptance tests, the recommended order is:

1. reject unsupported `schema_version`
2. reject operational failures from scanning, probing, or artifact inspection
3. inspect diagnostics and stop when required declarations or ABI-sensitive surfaces are marked as
   partial, unsupported, or otherwise unsafe for generation
4. require probed layouts for ABI-sensitive records, enums, typedef-backed variables, and other
   by-value surfaces that `fol` intends to generate as layout-sensitive bindings
5. reject validation reports with:
   - `AbiShapeMismatch`
   - `Missing`
   - `UnresolvedDeclaredLinkInputs`
   - `DuplicateProviders`
6. require the resolved link plan to have concrete provider paths for every required native input

That policy is intentionally narrower than the full generic LINC contract.
It is a recommended `fol` profile, not a universal rule for every downstream consumer.

## Minimal vs Extended `fol` Trust Surface

The current recommendation is to separate two `fol` modes clearly.

Minimal declaration-oriented flow:

- trust `BindingPackage.items`
- trust `schema_version`
- require inspection of `diagnostics`
- do not assume ABI-sensitive generation is ready from declarations alone

Extended ABI/link-aware flow:

- require `BindingPackage.layouts` for ABI-sensitive types
- require `ValidationReport` when native symbol presence and ABI shape matter
- require `ResolvedLinkPlan` when final native dependency assembly matters
- treat any unresolved or ambiguous provider state as a generation gate until `fol` has explicit
  higher-level policy for that case

## What `fol` Should Consider Ready

For the current integration profile, `fol` can treat a package as ready for high-confidence
generation only when all of the following are true:

- the scanned package deserializes under the expected schema version
- required declarations are present
- required layouts are present for ABI-sensitive generated surfaces
- diagnostics do not contain generation-blocking findings under `fol` policy
- validation does not report ABI-shape mismatch, missing providers, unresolved declared link
  inputs, or duplicate providers for required native declarations
- the link plan resolves every required native input to at least one concrete provider artifact

That keeps the `fol` contract narrow, explicit, and regression-testable while still leaving the
core LINC library contract general enough for other consumers.

## Minimal Durable Contract

If the integration needs a smallest stable contract first, it should standardize on:

- `BindingPackage`
- `schema_version`
- diagnostic handling rules
- optional `ValidationReport` for artifact-backed flows

That is the narrowest practical boundary that still scales toward fuller binder/linker behavior.
