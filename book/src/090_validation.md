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

Validation currently focuses on bindable symbol presence and kind.
It compares:

- extracted functions
- extracted variables

against:

- normalized symbol names
- raw/decorated symbol spellings
- symbol visibility
- symbol binding strength
- provider artifact provenance

## Match Statuses

Current statuses are:

- `Matched`
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

## CLI Usage

```sh
bic validate \
  --bindings-json bindings.json \
  --artifact build/libfoo.so \
  --artifact build/libsupport.a
```

The command prints a JSON `ValidationReport`.

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

## What Validation Does Not Prove

Current validation is symbol-oriented, not full ABI-signature verification.

A `Matched` status does not prove:

- function parameter ABI identity
- structure layout compatibility
- calling convention identity beyond the available evidence
- semantic compatibility across versions

Use validation as strong symbol-level evidence, not as a complete ABI theorem.
