# `fol` Extended Contract

This chapter defines the richer optional contract that `fol` may consume when it wants stronger
evidence than the minimal package surface alone.

## Extended Optional Inputs

The extended contract may include:

- `BindingPackage.layouts`
- `BindingPackage.link`
- captured macros and macro categories
- `SymbolInventory`
- `ValidationReport`

## Why This Contract Is Optional

These inputs are valuable, but they are not equally mature across all platforms and workflows.

That means `fol` should consume them opportunistically and explicitly, rather than assuming they
are always present or equally authoritative.

## Recommended Uses

### `layouts`

Use when:

- generated bindings depend on ABI-sensitive size/alignment facts
- record or alias representation needs stronger evidence than extraction alone

### `link`

Use when:

- the downstream flow needs declared native dependency intent
- library/framework/artifact ordering matters
- platform applicability needs to be preserved

### `SymbolInventory` and `ValidationReport`

Use when:

- the downstream flow wants artifact-backed evidence that declarations have real providers
- duplicate, hidden, or unresolved-provider states must gate release

### macros

Use when:

- constant lowering is attempted
- compile-time environment or ABI-affecting flags must be audited

## Consumer Rule

The safe rule for `fol` is:

- minimal contract is the baseline
- extended contract strengthens confidence, but should be treated field-by-field according to the
  documented maturity of each evidence layer

## Failure Posture

If extended evidence is required by policy and missing, `fol` should treat that as a consumer-side
policy failure, not as proof that `bic` is broken.

That distinction keeps the producer/consumer contract honest.
