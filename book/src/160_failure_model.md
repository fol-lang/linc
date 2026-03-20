# Failure Model

This chapter defines the intended behavior boundary between hard failures, diagnostics, and validation findings.

This is important because typed errors alone do not solve API clarity if consumers still do not know what should fail versus what should be reported as structured analysis output.

## The Three Outcome Classes

LINC currently has three practical outcome classes:

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

These are cases where LINC can still return a meaningful package, but the result is incomplete, lossy, or carries analysis warnings.

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

## Partial-Success Semantics

The important production rule is that successful return and acceptable return are different questions.

A practical consumer decision flow is:

1. if the operation returned `Err(...)`, treat it as an execution failure
2. if the operation succeeded, inspect `BindingPackage.diagnostics`
3. if layouts were expected, verify that the expected `package.layouts` evidence is present
4. if native artifacts matter, run validation and inspect the resulting `ValidationReport`
5. only then decide whether generation should continue

This means a robust downstream integration should not collapse everything into a single boolean
"success" value.

### When Diagnostics Should Block Generation

In most real pipelines, diagnostics should be treated as blocking when they indicate:

- unsupported declarations that are required by the generated binding surface
- partial extraction of ABI-relevant constructs
- preprocessing or parse recovery that leaves the package materially incomplete

Diagnostics may be non-blocking when they affect declarations that the downstream generator does
not intend to expose.

### When Validation Findings Should Block Linking Or Publication

Validation findings should usually block the next stage when they show:

- missing required symbols
- duplicate visible providers for the same declaration
- unresolved declared native inputs
- visibility or decoration mismatches that make the selected provider ambiguous

Validation findings are still structured output rather than execution failure, but they are often
release-blocking evidence.

## Consumer Decision Table

| Outcome | Meaning | Typical downstream response |
|---|---|---|
| `Err(...)` | operation could not produce a usable result | stop immediately |
| success + no important diagnostics | analysis succeeded cleanly enough | continue |
| success + diagnostics | analysis completed, but may be partial or lossy | review or gate |
| success + validation findings | artifact comparison completed and found mismatches | block publish/link until resolved |

## Why This Distinction Matters

Without this distinction, downstream consumers tend to make one of two mistakes:

- treating diagnostics as harmless warnings when they may indicate unusable bindings
- treating validation mismatches as exceptional control flow rather than structured evidence

This chapter makes the intended split explicit before later slices migrate more APIs to typed errors.
