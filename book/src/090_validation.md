# Validation

Validation compares a `BindingPackage` against one or more `SymbolInventory` values.

This answers a practical question:

> do the declarations we extracted line up with what the native artifacts actually provide?

## API Entry Points

Single artifact:

```rust
use bic::validate;

let report = validate(&package, &inventory);
```

Multiple artifacts:

```rust
use bic::validate_many;

let report = validate_many(&package, &[inv_a, inv_b]);
```

Use `validate_many` whenever the package is expected to resolve across more than one object/archive/shared library.

## What Validation Looks At

Validation currently focuses on bindable symbol presence and kind, with conservative ABI-shape evidence where the library can prove something honestly.
It compares:

- extracted functions
- extracted variables

against:

- normalized symbol names
- raw/decorated symbol spellings
- symbol visibility
- symbol binding strength
- routine parameter-count hints
- provider artifact provenance

## Validation Phases

`ValidationReport.phases` exposes the current validation pipeline as explicit report data:

- `ProviderDiscovery`
- `SymbolIdentity`
- `AbiEvidence`

Today, the first two phases complete during normal symbol validation and `AbiEvidence` remains
present but incomplete. That is intentional: consumers can now distinguish the current
symbol-resolution coverage from the deeper ABI-evidence work that is still evolving.

## Match Statuses

Current statuses are:

- `Matched`
- `AbiShapeMismatch`
- `Missing`
- `UnresolvedDeclaredLinkInputs`
- `DecorationMismatch`
- `NotAFunction`
- `NotAVariable`
- `Hidden`
- `WeakMatch`
- `DuplicateProviders`

Each status tells you something different.

## Status Semantics

### `Matched`

The declaration name resolved to a visible symbol of the expected kind.

For variable declarations, `Matched` may now carry positive ABI-shape evidence when `bic`
can compare an observed symbol size against an inferred expected size.

For function declarations, `Matched` may now also carry positive routine ABI evidence when
the provider inventory exposes conservative routine hints and they agree with the extracted
declaration. Today that starts with:

- parameter count
- primitive return-size shape where both sides expose a trustworthy size
- by-value parameter-size shape where both sides expose trustworthy sizes

### `AbiShapeMismatch`

The declaration resolved to a visible provider of the expected broad kind, but the available
artifact-side size metadata disagreed with the expected declaration-side size.

This is intentionally limited today:

- it only applies where artifact metadata exposes a usable size
- it only applies where `bic` can infer an expected size conservatively
- routine checks are still conservative: they currently cover parameter count, primitive
  return-size shape, and by-value parameter-size shape, not full calling-convention or
  register-level proof

### `Missing`

No matching symbol was found and the package did not even declare native link inputs that might reasonably have provided it.

### `UnresolvedDeclaredLinkInputs`

No symbol was found, but the package did declare native link inputs.

This is a stronger signal than plain missing because it tells you the package expected a provider and the validation step failed to find it.

### `DecorationMismatch`

No normalized symbol matched, but a decorated/raw spelling normalized to the declaration name.

This is the "something probably exists, but the name spelling path does not line up cleanly" status.

### `NotAFunction` and `NotAVariable`

A symbol exists, but it has the wrong broad kind for the declaration.

### `Hidden`

A candidate symbol exists, but it is not visible as a usable exported provider.

### `WeakMatch`

The symbol is present, but only as a weak binding.

This is often acceptable evidence, but it should not be treated as equivalent to a strong exported provider in every workflow.

### `DuplicateProviders`

More than one visible provider satisfies the same declaration across the validated artifact set.

This is especially important when merging link surfaces from several native inputs.

## Provider Artifacts

Each `SymbolMatch` can record `provider_artifacts`.

That list may include:

- plain artifact paths
- archive member provenance such as `libfoo.a:bar.o`

This is useful for:

- debugging multi-artifact validation
- surfacing duplicate providers
- building later link-resolution heuristics

## Richer Report Entries

`ValidationReport.entries` is the richer structured surface.
Each `ValidationEntry` preserves:

- the declaration identity (`name`, `item_kind`)
- the resulting `status`
- attached `ValidationEvidence`

`ValidationEvidence` currently keeps:

- `provider_artifacts`
- `raw_symbol_names`
- observed `visibility`
- `confidence`
- `evidence_kind`
- `abi_shape` for variable-size evidence
- `routine_abi` for conservative routine-shape evidence

Routine ABI evidence now carries its own explicit shape classification and confidence ladder so
consumers can distinguish:

- partial routine evidence such as parameter count only
- stronger routine evidence such as count plus return/parameter shape
- mismatched routine evidence where the symbol exists but the observed routine shape disagrees

The older `matches` list remains available as the flatter compatibility surface.

## Confidence

Validation confidence is currently a policy-friendly summary of the available symbol evidence:

- `High`: direct visible provider of the expected kind
- `Medium`: provider exists, but only as a weak symbol
- `Low`: partial or conflicting evidence exists
- `None`: no provider evidence was found

## Evidence Kind

`evidence_kind` is the more structural classification of provider state.
Current values distinguish:

- exact exported providers
- weak exported providers
- hidden providers
- decorated-name candidates
- re-export candidates inferred from dependency-bearing shared artifacts
- duplicate visible providers
- declared link inputs without a discovered provider
- plain missing providers
- ABI-shape-verified providers
- ABI-shape mismatches
- wrong-kind providers

`ReexportedCandidate` can now come from either:

- a broad artifact-level dependency signal
- a symbol-local imported entry that carries `reexported_via`

## How To Read A Report

The most important first pass is:

1. count `Matched`
2. count `Missing` and `UnresolvedDeclaredLinkInputs`
3. inspect `DuplicateProviders`
4. inspect `DecorationMismatch`

That ordering usually tells you whether the problem is:

- missing native inputs
- unexpected symbol spelling
- duplicate/native-surface ambiguity
- visibility or kind mismatch

## Summary And Helpers

`ValidationReport.summary` gives a stable count-oriented surface for policy code.
It currently tracks totals for:

- matched declarations
- missing declarations
- unresolved declared link inputs
- hidden providers
- weak matches

For downstream code that wants to reason about trust rather than only status, use the helper
surface instead of decoding optional fields by hand:

- `ValidationEvidence::has_layout_backed_confidence()`
- `ValidationEntry::has_layout_backed_confidence()`
- `ValidationReport::layout_backed_entries()`
- `ValidationEntry::{has_resolved_provider_state, has_unresolved_provider_state, has_ambiguous_provider_state}`
- `ValidationReport::{resolved_provider_entries, unresolved_provider_entries, ambiguous_provider_entries}`
- duplicate providers
- decoration mismatches
- kind mismatches

The report also exposes helper methods such as:

- `matched()`
- `missing()`
- `hidden()`
- `weak_matches()`
- `duplicate_providers()`
- `unresolved_declared()`

The repository also keeps dedicated validation regression fixtures for ambiguous-provider cases so
duplicate-provider evidence remains stable across JSON and API evolution.

## What Validation Does Not Prove

Current validation is symbol-oriented, not full ABI-signature verification.

A `Matched` status does not prove:

- function parameter ABI identity
- structure layout compatibility
- calling convention identity beyond the available evidence
- semantic compatibility across versions

The current ABI-evidence layer is intentionally narrow:

- variable size checks from object metadata
- primitive-type expectations
- layout-backed expectations when `BindingPackage.layouts` provides the relevant named type size

Use validation as strong symbol-level evidence, not as a complete ABI theorem.
