# JSON Artifacts

This chapter describes how `linc` treats serialized JSON artifacts.

The important framing is architectural:

- JSON is an artifact format
- it is not a promise to preserve old pipeline shapes forever
- the only shapes that matter are the ones currently documented and tested

`linc` does not carry a backward-compatibility burden for discarded designs.

## First Principle

The artifact contract is about semantic meaning, not pretty-print formatting.

Consumers may depend on:

- documented field names
- documented field meanings
- `schema_version`
- documented defaulting behavior when tests rely on it

Consumers must not depend on:

- whitespace
- pretty-print layout
- incidental field ordering
- undocumented fields that happen to be present today

## Main Serialized Artifacts

The main JSON-bearing values are:

- `LinkAnalysisPackage`
- `SymbolInventory`
- `ValidationReport`
- `ResolvedLinkPlan`

Older package names or historical envelopes are not the contract.

## Version Fields

Two version-like fields matter:

- `schema_version`
- `linc_version`

They do different jobs.

### `schema_version`

`schema_version` is the artifact gate.

Consumers should use it to decide whether they understand the artifact shape
they are about to ingest.

Rules:

- if `schema_version` is newer than the consumer understands, reject it
- if the shape changes in a meaning-changing way, review it explicitly and update tests
- if a field is only additive and has a safe default, document that choice and test it

### `linc_version`

`linc_version` identifies the producing build.

It is useful for:

- diagnostics
- audits
- bug reproduction
- provenance

It is not the primary shape gate.

## Change Policy

Because these crates are still new, the policy is intentionally strict and
simple:

- do not preserve obsolete artifact envelopes just because they existed earlier
- do not keep old field layouts alive unless current tests still need them
- do make semantic changes explicit in docs and fixtures
- do update `schema_version` when consumers would otherwise misread the artifact

## Producer Guidance

If a tool emits `linc` JSON:

1. preserve `schema_version` exactly
2. serialize the documented artifact, not an old transitional wrapper
3. keep fixture coverage in sync with newly relied-on fields
4. treat `linc_version` as provenance, not as the shape contract

## Consumer Guidance

If a tool consumes `linc` JSON:

1. check `schema_version`
2. deserialize into the currently documented structure
3. rely on documented semantics only
4. reject future shapes instead of guessing

## What Counts As A Real Shape Change

These usually require explicit review and likely a schema bump:

- changing the meaning of an existing field
- removing a relied-on field
- changing representation so an older consumer would misread the artifact
- promoting previously incidental data into required semantics

These may stay within the same schema line if documented and tested:

- adding a new field with a safe default
- adding new metadata that older consumers can ignore without semantic confusion
- clarifying documentation without changing meaning

## Maintenance Rule

The tests are the practical statement of the JSON contract.

When artifact shapes change:

1. update the docs
2. update or replace the relevant fixtures
3. update consumers in tests/examples/harnesses
4. do not carry a dead compatibility layer just to keep an obsolete shape deserializable
