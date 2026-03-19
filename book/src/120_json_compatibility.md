# JSON Compatibility

This chapter defines the intended compatibility model for serialized `BindingPackage` values.

That matters because `BindingPackage` is the main machine-to-machine contract between `bic` and downstream consumers such as `fol`.

## First Principle

The compatibility contract is about the meaning of the data, not the exact pretty-printed formatting.

Consumers should depend on:

- field names
- field meanings
- schema-version behavior
- documented defaulting behavior

Consumers should not depend on:

- whitespace
- formatting layout
- incidental field ordering in pretty JSON for semantic correctness

## Version Fields

Two version-like fields exist in the package:

- `schema_version`
- `bic_version`

They do different jobs.

## `schema_version`

`schema_version` is the compatibility gate.

Downstream tools should use it to decide whether they understand the payload contract.

Rules:

- if `schema_version` is newer than this build of `bic` supports, deserialization should fail
- if `schema_version` is older, deserialization may still succeed when missing fields are intentionally defaultable
- schema changes should be deliberate, documented, and fixture-tested

## `bic_version`

`bic_version` identifies the producing crate version.

It is useful for:

- diagnostics
- auditing
- reproducing bugs
- understanding producer provenance

It is not the primary compatibility key.

## Backward Compatibility Expectations

The intended backward-compatibility policy for additive evolution is:

- new optional fields should use serde defaults when old payloads can reasonably omit them
- older payloads should continue to deserialize when their missing data can be represented safely by defaults
- changes that alter meaning rather than only adding optional data should be treated as schema changes, not silent field growth

Current examples already following this pattern include defaultable nested metadata such as:

- `target`
- `inputs`
- `link`
- macro category defaults

## Forward Compatibility Expectations

Forward compatibility is intentionally conservative.

If a payload advertises a future `schema_version`, `bic` should reject it rather than guessing.

That is safer than partially interpreting a payload whose semantics may have changed.

## What Counts As A Schema Change

These cases should generally force explicit compatibility review and likely a schema-version bump:

- changing the meaning of an existing field
- removing a field that downstream consumers may rely on
- changing representation in a non-defaultable way
- tightening semantics such that old values would be misinterpreted

These cases may be compatible without a schema bump if handled deliberately:

- adding a new field with a safe default
- adding new metadata that older consumers can ignore
- clarifying documentation without changing wire meaning

## Consumer Guidance

If another tool stores or consumes `BindingPackage` JSON, it should:

1. check `schema_version`
2. deserialize into documented structures
3. rely only on documented semantics
4. avoid depending on undocumented incidental formatting details

For `fol` specifically, this means:

- the package should be treated as a versioned data contract
- any new relied-on field should be documented explicitly before it becomes part of the stable integration contract

## Current Limitations

This compatibility policy is still early.

Today:

- `SCHEMA_VERSION` is still conservative relative to how much the IR has grown
- not every field has been formally classified as stable vs provisional
- fixture coverage for schema evolution still needs to expand

That is why compatibility policy is being established before more major IR changes land.
