# Failure Model

This chapter defines the intended behavior boundary between hard failures, diagnostics, and validation findings.

This is important because typed errors alone do not solve API clarity if consumers still do not know what should fail versus what should be reported as structured analysis output.

## The Three Outcome Classes

`bic` currently has three practical outcome classes:

1. hard operational failure
2. successful analysis with diagnostics
3. successful validation with findings

## 1. Hard Operational Failure

These are failures where the requested operation could not produce a meaningful result.

Examples:

- unreadable input files
- schema-version mismatch during JSON import
- preprocessing or parse failure severe enough that no usable analysis result can be produced
- probe execution failure
- artifact inspection failure

These should be represented as returned errors.

## 2. Successful Analysis With Diagnostics

These are cases where `bic` can still return a meaningful package, but the result is incomplete, lossy, or carries analysis warnings.

Examples:

- unsupported source constructs that are recognized but not fully modeled
- best-effort extraction where fidelity is partial
- source features that are reduced to diagnostics instead of hard failure

These should appear in:

- `BindingPackage.diagnostics`

The important consumer rule is:

- a successful return does not automatically mean "safe to generate without review"

## 3. Successful Validation With Findings

Validation is not a hard-error channel for normal mismatches.

Examples:

- missing symbols
- hidden symbols
- decoration mismatches
- duplicate providers
- unresolved declared link inputs

These should appear in:

- `ValidationReport`

The important consumer rule is:

- validation findings are analytical results, not transport failure

## Current Practical Guidance

Downstream code should currently interpret the library like this:

- `Err(...)` means the requested operation itself failed
- diagnostics mean the operation succeeded, but the returned analysis may be partial or lossy
- validation findings mean the operation succeeded and produced evidence that the native surface does not match expectations cleanly

## Why This Distinction Matters

Without this distinction, downstream consumers tend to make one of two mistakes:

- treating diagnostics as harmless warnings when they may indicate unusable bindings
- treating validation mismatches as exceptional control flow rather than structured evidence

This chapter makes the intended split explicit before later slices migrate more APIs to typed errors.
