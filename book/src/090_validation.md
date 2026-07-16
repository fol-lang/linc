# Validation

Validation compares a `BindingPackage` against one or more `SymbolInventory`
values.

It reports how declarations line up with the symbol inventories supplied to
the call and, when present, limited shape observations. It does not answer the
broader question of full ABI compatibility.

## API Entry Points

Use `validate` for one artifact and `validate_many` for several.

## What Validation Looks At

Validation focuses on symbol presence, symbol kind, visibility, binding
strength, decorated names, and conservative ABI-shape evidence where the
artifact can prove something honestly.

## Common Statuses

Current statuses include:

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

## How To Read A Report

- `Matched` means a visible exported symbol with the same normalized name and
  expected function/variable kind was found
- `Missing` means no matching symbol was found and the package did not declare
  native link inputs that might reasonably have provided it
- `UnresolvedDeclaredLinkInputs` means the package did declare native inputs,
  but validation still found no provider
- `DecorationMismatch` means a decorated or raw spelling normalized to the
  declaration name
- `Hidden` and `WeakMatch` should usually be treated more conservatively than a
  strong export
- `DuplicateProviders` usually blocks promotion until the consumer chooses a
  policy

## Provider Evidence

Provider evidence may include plain artifact paths or archive-member provenance
such as `libfoo.a:bar.o`.

Those strings identify the supplied inventory and are not independent proof of
provider authenticity, target compatibility, linkability, or runtime
availability.

## ABI Boundary

A matched name is not ABI validation. Some entries also carry variable-size,
parameter-count, return-size, or parameter-size comparisons; inspect
`evidence_kind`, `abi_shape`, `routine_abi`, and the validation phases before
using those observations. Even the strongest current shape evidence does not
prove calling conventions, aggregate classification, variadics, every target
ABI rule, or behavior.

## Consumer Rule

Validation findings are structured evidence, not hard execution errors. Treat
them as policy input for the next stage.
