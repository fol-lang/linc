# `fol` Integration Guide

This chapter describes the intended producer/consumer contract between `bic` and `fol`.

`bic` should be read as the C-analysis producer.
`fol` should be read as the downstream consumer that decides what to generate, what to reject, and
how to assemble its final binding/link workflow.

## Division Of Responsibility

The intended split is:

- `bic` extracts declarations, metadata, diagnostics, layouts, and native-surface evidence
- `fol` consumes that evidence to generate bindings, choose generation policy, and drive any final
  language/runtime-specific integration

`bic` is not expected to replace `fol`.
It is expected to provide a stable machine contract that `fol` can trust.

## What `bic` Should Produce For `fol`

For a healthy integration, `fol` should expect `bic` to provide:

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

1. run `bic` header scanning or preprocessed parsing
2. inspect `BindingPackage.diagnostics`
3. require layout probes for ABI-sensitive types
4. inspect native artifacts and run validation when linkable symbols matter
5. serialize or pass the resulting structured values to `fol`
6. let `fol` decide what to generate, reject, or gate behind policy

## Contract Boundaries

The key contract boundaries are:

- `schema_version` is the wire-compatibility gate
- `bic_version` is producer provenance
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

These gates should be policy decisions in `fol`, but the evidence should come from `bic`.

## Minimal Durable Contract

If the integration needs a smallest stable contract first, it should standardize on:

- `BindingPackage`
- `schema_version`
- diagnostic handling rules
- optional `ValidationReport` for artifact-backed flows

That is the narrowest practical boundary that still scales toward fuller binder/linker behavior.
