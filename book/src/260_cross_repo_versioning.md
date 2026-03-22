# Cross-Repo Versioning

This chapter defines the coordination policy between LINC and downstream consumers such as `fol`.

The goal is to avoid accidental wire-contract drift across repositories.

## Artifact Keys

The intended keys are:

- `schema_version` for machine-contract consumption
- crate/repo versions for provenance and release coordination

`schema_version` is the primary technical artifact gate.

## Coordination Rules

When LINC changes a relied-on contract:

1. document the change
2. update fixtures/snapshots
3. decide whether the change is additive/defaultable or schema-breaking
4. coordinate any `fol` reliance change explicitly

## Additive Changes

For additive/defaultable changes:

- `schema_version` may remain stable
- `fol` may opt into the new field or evidence later
- snapshots and docs should still be updated

## Breaking Contract Changes

For breaking changes:

- `schema_version` should change deliberately
- `fol` should not silently consume the new payload as if nothing changed
- fixture updates should land in a coordinated manner

## Consumer Rule

`fol` should treat new LINC producer versions as:

- consumable if the relied-on `schema_version` contract is still supported
- potentially behaviorally different if new optional fields appear
- not consumable if the supported schema contract changes beyond what `fol`
  understands

## Recommended Workflow

The safest cross-repo workflow is:

1. land LINC contract/documentation updates first
2. update or add contract fixtures
3. update `fol` to consume the new contract deliberately
4. keep acceptance tests pinned to explicit fixture payloads where possible
