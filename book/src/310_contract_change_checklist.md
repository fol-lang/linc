# Contract Change Checklist

Use this checklist whenever a release includes changes to schema, public API, or checked-in
contract fixtures.

The goal is to make contract-impacting changes deliberate rather than accidental.

## Schema Changes

Before releasing a schema-affecting change:

- confirm whether the change is additive, behavioral, or breaking
- keep `schema_version` unchanged for additive/defaulted changes
- bump `schema_version` only when older consumers can no longer deserialize or safely interpret the
  payload
- add or update contract fixtures that demonstrate the intended behavior
- only keep older fixture shapes when the current documented artifact still claims to accept them

## Public API Changes

Before releasing a public API change:

- confirm whether the root-level API contract changed or only lower-level modules changed
- update crate-level docs and book chapters when recommended usage changes
- keep stable entry-point guidance current
- add or update public API coverage if a new root-level behavior is now part of the contract

## Fixture Changes

Before releasing fixture changes:

- confirm the fixture still represents a real supported or intentionally unsupported scenario
- confirm the corresponding regression test explains why the fixture exists
- remove stale fixtures only when the underlying contract surface is intentionally removed or
  replaced

## Consumer Guidance Changes

Before releasing consumer-guidance changes:

- confirm generic library guidance stays separate from consumer-specific guidance
- confirm `fol` guidance remains an example consumer profile rather than the universal crate policy
- confirm readiness/limitation wording still matches the actual regression boundary

## Final Questions

Ask these before tagging a release:

- did this change alter what downstream code can safely rely on?
- did fixture coverage change to prove that new reliance boundary?
- did the docs change to describe the same boundary?
- would a consumer that still claims to understand this schema line misread the new payload or semantics?

If the answer to any of those questions is "yes" and the release notes do not explain it yet,
the release is not ready.
