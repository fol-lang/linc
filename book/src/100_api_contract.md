# API Contract

This chapter defines the current intended public library surface of LINC.

It is not yet a semver policy document for every future release.
It is the current explicit contract for how downstream consumers should approach the crate.

## First Principle

LINC is a library crate.

The intended downstream pattern is:

1. call the crate from Rust
2. obtain structured values such as `LinkAnalysisPackage`, `SymbolInventory`, and `ValidationReport`
3. serialize those values only when another tool or process boundary needs them

Consumers should prefer the crate root over deep module imports whenever possible.

## Crate API Policy

The current intended crate policy is:

- the crate root is the preferred consumer boundary
- public modules may still be used directly, but module depth correlates with how implementation-shaped an API is
- additive, documented evolution is preferred over disruptive surface churn
- typed data contracts are more important than incidental formatting or helper layout
- diagnostics and validation reports are contractual structured output, not just debugging aids

This policy should guide both downstream usage and future maintenance work.

## Normative Rules For Consumers

If you are building on top of LINC, the current intended rules are:

1. prefer crate-root re-exports over deep module imports
2. use `analyze_source_package` as the normal contract-first entry point
3. treat `LinkAnalysisPackage`, `SymbolInventory`, and `ValidationReport` as the primary transport-level contracts
4. treat diagnostics and validation results as normal structured output, not as ad hoc log text
5. do not rely on exact `String` error text for durable control flow
6. do not treat extracted declarations alone as sufficient ABI proof for layout-sensitive generation

These rules are the safest current downstream posture until later API and error-model slices land.

## Stability Tiers

The public surface is best understood in three tiers.

## Tier 1: Preferred Root-Level API

These are the APIs downstream users should prefer first.

| API | Role | Current expectation |
|---|---|---|
| `analyze_source_package` | source-contract to link-analysis contract | preferred public entry point |
| `LinkAnalysisPackage` | machine-readable link-analysis contract | preferred public contract |
| `HeaderConfig` | raw-header scanning | transitional bootstrap entry point |
| `BindingPackage` and re-exported IR types | migration bridge and low-level IR | transitional public contract |
| `probe_type_layouts` | compiler-assisted ABI evidence | preferred advanced root API |
| `inspect_symbols` | native artifact inventory | preferred advanced root API |
| `validate` / `validate_many` | declaration-vs-artifact checks | preferred advanced root API |

This tier is what later API-stability work should protect most aggressively.

## Tier 2: Advanced Public Modules

These modules are public and useful, but they are closer to the implementation.

| Module | Why it is public | Why it is not the first choice |
|---|---|---|
| `extract` | useful for direct extraction flows | lower-level than crate-root workflows |
| `probe` | useful for direct probe control | less curated than root API |
| `raw_headers` | exposes scan orchestration details | crate root already re-exports the important types |
| `symbols` | useful for direct artifact work | implementation-shaped details still live here |
| `validate` | useful for direct report logic | root re-exports are preferred |

These modules are valid to use.
They are simply not the most stable-looking consumer surface yet.

## Tier 3: Support-Oriented Public Modules

These modules are public today, but consumers should only depend on them deliberately.

| Module | Notes |
|---|---|
| `diagnostics` | useful when inspecting detailed extraction output |
| `error` | defines crate error types, still maturing |
| `ir` | canonical raw IR definitions, but still evolving |
| `line_markers` | low-level origin tracking support |
| `preprocess` | preprocessed-input support details |

If a downstream consumer imports heavily from this tier, it is probably depending on details that later cleanup work may want to simplify.

## What Downstream Users Should Prefer

Prefer:

- crate-root re-exports
- `analyze_source_package` for contract-first intake
- `LinkAnalysisPackage` as the durable downstream link contract
- root-level validation and symbol APIs
- `HeaderConfig` only when bootstrapping from raw headers inside the repo

For long-lived downstream integrations, also prefer:

- documented behavior over inferred behavior
- package-level metadata over reconstructing intent from raw declarations alone
- package diagnostics and validation output as explicit decision inputs

Avoid reaching for deep modules first unless:

- you are building advanced integration code
- you need lower-level control not exposed at the crate root
- you are contributing to LINC itself

## Current Sharp Edges

This inventory is honest about the present state.
The following are still true today:

- some public APIs still return `Result<_, String>`
- some module boundaries are more historical than deliberate
- the root exports a large raw IR surface because downstream tools genuinely need it
- `BindingPackage` is still present as migration scaffolding

That is why the next plan phase starts with API cleanup and error-model hardening.

## Immediate Consumer Guidance

If you are integrating LINC into another crate, treat the following as your safest surface:

1. root-level types and functions
2. serialized `LinkAnalysisPackage` / `SymbolInventory` / `ValidationReport` values
3. book-level documented behavior, not incidental implementation details

If you need more than that, document exactly which lower-level modules you rely on.
That will make later stabilization work much easier.

## Type Invariants

Public structs and enums now carry invariant-oriented docs in the source.

Those notes are part of the library contract.
They explain things like:

- which fields are identity keys versus optional evidence
- which vectors preserve declaration order
- which normalized values are not full linker or ABI truth
- which report types represent successful analysis with findings rather than hard failures

For durable integrations, read those source-level invariant notes as part of the supported API.

## Explicit Non-Guarantees

The current contract does not yet guarantee:

- typed operational errors across the whole crate
- full ABI completeness for all C constructs
- full cross-platform parity across ELF, Mach-O, and Windows-native artifact formats
- that every public module is equally stable as a consumer boundary

These are roadmap items, not present-tense promises.
