# `fol` Minimal Contract

This chapter defines the smallest contract that `fol` should be allowed to rely on without taking
on unnecessary fragility.

## Minimal Required Inputs

The minimal durable contract is:

- `BindingPackage`
- `schema_version`
- `BindingPackage.items`
- `BindingPackage.diagnostics`

That is the narrowest practical producer/consumer boundary that still supports basic binding
generation decisions.

## Minimal Required Semantics

`fol` may rely on these rules:

- `schema_version` is the compatibility gate
- a successful scan may still contain diagnostics that matter
- `BindingPackage.items` is the extracted declaration surface
- diagnostics are part of the data contract, not incidental logs

## What The Minimal Contract Does Not Promise

The minimal contract does not promise:

- full ABI confidence
- native artifact validation
- complete linker resolution
- availability of layout evidence for every type
- that every macro can be lowered safely

## Why Keep The Minimal Contract Narrow

A narrow minimal contract makes the integration more stable because:

- it minimizes cross-repo coupling
- it avoids forcing `fol` to depend on still-maturing evidence layers
- it allows richer optional data to grow additively

## Recommended `fol` Behavior On The Minimal Contract

When only the minimal contract is available, `fol` should:

1. check `schema_version`
2. inspect diagnostics
3. decide whether generation is allowed for the requested declarations
4. avoid making ABI-strong claims that require missing evidence
