# Field Stability

This chapter classifies the current `BindingPackage` contract by stability expectations.

It does not mean every field is forever frozen.
It means downstream consumers now have an explicit guide for what is safer to rely on.

## Top-Level `BindingPackage`

The top-level package fields fall into three practical groups:

- contract identity fields
- stable container fields
- evolving evidence fields

## Contract Identity Fields

These are the primary package identity and compatibility fields:

| Field | Current classification | Notes |
|---|---|---|
| `schema_version` | required contract field | compatibility gate |
| `linc_version` | stable provenance field | producer version, not the main compatibility key |
| `source_path` | useful provenance field | helpful, but not the primary compatibility anchor |

## Stable Container Fields

These are the major package sections downstream tools can reasonably depend on existing:

| Field | Current classification | Notes |
|---|---|---|
| `target` | stable container | nested contents may grow |
| `inputs` | stable container | nested contents may grow |
| `macros` | stable container | macro modeling still evolving |
| `layouts` | stable container | probe richness still evolving |
| `link` | stable container | resolution semantics still evolving |
| `items` | stable container | declaration modeling still evolving |
| `diagnostics` | stable container | taxonomy may still deepen |

The important distinction is:

- the container concepts are stable enough to build around
- not every nested field should be assumed permanently frozen yet

## Nested Stability Guidance

## `target`

Current nested fields such as target triple, compiler command, compiler version, and flavor should be treated as:

- stable descriptive metadata
- additive/defaultable

Consumers may rely on the presence of the `target` object.
They should not assume no new target metadata will ever be added.

## `inputs`

The `inputs` object should be treated as:

- stable input provenance container
- additive/defaultable

Entry headers, include dirs, and defines are safe concepts to consume.
The exact set of input metadata may still grow.

## `macros`

Macro inventory is a stable concept.
Its precise semantic richness is still evolving.

Consumers may safely rely on:

- the existence of macro inventory
- macro names and bodies being preserved when captured

Consumers should be cautious about treating the current kind/category set as the final maximum taxonomy.

## `layouts`

Layout evidence is a stable concept.
The current `TypeLayout` shape is still minimal.

Consumers may rely on:

- package-level layout evidence existing
- current size/align data when present

Consumers should not assume this is the full eventual ABI layout model.

## `link`

The normalized link surface is a stable concept and a core downstream contract area.

Consumers may rely on:

- package-level link metadata existing
- libraries/frameworks/artifacts/ordered inputs being preserved as concepts

Consumers should expect:

- additional metadata
- richer resolution semantics
- future resolved-planning data in addition to the current descriptive surface

## `items`

Binding items are the core declaration payload and are stable as a container concept.

Consumers may rely on:

- function, record, enum, typedef, variable, and unsupported item variants existing as current categories

Consumers should still assume nested type/declaration fidelity will deepen over time.

## `diagnostics`

Diagnostics are a stable concept.
Their taxonomy and richness may still expand.

Consumers may rely on:

- diagnostics being part of the returned package
- diagnostics representing analysis findings rather than transport failure

Consumers should avoid assuming the current set of diagnostic kinds is final.

## Practical Rule For Downstream Consumers

Today, the safest posture is:

1. rely on top-level package sections and documented meanings
2. treat nested metadata as additive/defaultable unless explicitly documented as a compatibility gate
3. use `schema_version` as the hard compatibility boundary
4. avoid assuming current minimal models are final models

This is the right balance between "the contract is meaningless" and "every nested field is frozen forever."
