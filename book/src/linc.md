# LINC Reference

LINC is the link and binary evidence layer in the `parc -> linc -> gerc`
toolchain.

It owns evidence, not parsing and not lowering, but the crate surface today is
broader than the preferred top-level story. Both the contract-first APIs and
the older low-level IR/bootstrap APIs are still real.

LINC is in H0 hardening and is not production-certified. Its link plans are
normalized candidate plans, and its validation reports are symbol/shape
evidence rather than full ABI proof. See
[Hardening Status](./005_hardening_status.md) for the exact boundary.

## What LINC Is For

LINC turns normalized source intent into native evidence. It can:

- normalize declared link requirements
- inspect object, archive, and shared-library artifacts
- probe ABI-relevant layouts
- compare declarations with symbol and optional shape observations
- serialize the resulting evidence for downstream consumers

## What LINC Produces

The main outputs are:

- `LinkAnalysisPackage`
- `SymbolInventory`
- `ResolvedLinkPlan`
- `ValidationReport`
- `AbiProbeReport`

Those version-1 outputs are transportable evidence artifacts, not the frozen
H1 contract. The preferred modern
consumer path is `SourcePackage -> LinkAnalysisPackage`, but LINC also still
exposes `BindingPackage` and lower-level IR for direct inspection and staged
work.

## Data Flow

```text
normalized source input
  -> linc analysis
  -> link/binary evidence artifacts
  -> downstream consumer
```

In practice the preferred input is `SourcePackage`, the preferred analysis
entrypoint is `analyze_source_package`, and symbol/probe/validation are layered
evidence on top.

## Ownership Boundary

LINC owns:

- the evidence model
- the link surface
- the validation story
- the ABI probe story

LINC does not own:

- parser internals
- source preprocessing internals
- Rust code generation
- library-level composition with `parc` or `gerc`

Composition across packages belongs in tests, examples, or external harnesses.

## Modules And APIs

The root APIs are:

- `analyze_source_package`
- `inspect_symbols`
- `probe_type_layouts`
- `resolve_link_plan`
- `validate`

The root also still re-exports many low-level IR and report types, and tests
exercise those paths directly.

The important modules are:

- `intake`
- `analysis`
- `link_plan`
- `probe`
- `symbols`
- `validate`
- `diagnostics`
- `error`

`raw_headers` still exists for repo-local bootstrap and fixture work. It is not
the architectural center of the crate, but it is still a public low-level
surface that the book needs to acknowledge honestly.

## Reading Order

1. [Getting Started](./010_getting_started.md)
2. [Intake Layer](./015_intake.md)
3. [Header Processing](./020_headers.md)
4. [IR Model](./030_ir.md)
5. [Native Evidence](./095_native_evidence.md)
6. [API Contract](./100_api_contract.md)
7. [End-To-End Workflows](./110_workflows.md)
8. [Operations And Release](./205_operations_and_release.md)
