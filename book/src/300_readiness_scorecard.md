# Readiness Scorecard

This chapter summarizes current release readiness by subsystem.

It is not a semver guarantee by itself.
It is a concise statement of current confidence based on the present code, fixtures, and tests.

## Overall Readiness

For the current non-Windows scope, LINC should be read as:

- strong for library-first extraction and evidence production
- strong for ELF-oriented validation and link planning
- good for Mach-O inventory and conservative provider analysis
- intentionally conservative, not exhaustive, for final native-linker semantics
- backed by a stress-example ladder that now includes:
  - Linux system-header examples
  - real-library examples
  - plugin/runtime-loader examples
  - a combined daemon-style mixed surface

The main remaining open items after the current follow-up cycle are:

- host-header dependence for some Linux system examples
- host-package dependence for most of the real-library ladder beyond the vendored `zlib` baseline
- downstream deployment/runtime policy for plugin and mixed-subsystem activation paths

## Subsystem Scorecard

### Header Extraction

- readiness: high
- basis:
  - broad unit coverage
  - fixture-driven regression coverage
  - stable root-level entry points
  - stress-cycle recovery coverage for packed typedef record declarations

### JSON Contract

- readiness: high
- basis:
  - schema-version gate
  - artifact fixtures
  - additive defaulting on evolving fields

### ABI Layout Evidence

- readiness: medium-high
- basis:
  - probed layouts are integrated into scans and validation
  - typedef-backed and representation-backed checks exist
  - partial bitfield and record evidence is preserved
  - failed opaque/incomplete probe requests now degrade into diagnostics instead of discarding the
    rest of the scan
- remaining limit:
  - this is still evidence-driven ABI checking, not a full universal ABI proof engine

### Symbol Inventories

- readiness: high for ELF, medium-high for Mach-O
- basis:
  - export/import distinction
  - alias and re-export evidence
  - dependency edges
  - platform fixtures and regressions
- remaining limit:
  - Mach-O semantics are intentionally modeled conservatively rather than exhaustively

### Validation

- readiness: medium-high
- basis:
  - structured phases, entries, summaries, and evidence kinds
  - ABI-shape mismatch coverage
  - duplicate-provider and unresolved-provider handling
- remaining limit:
  - validation is not a full platform ABI/linker oracle

### Link Planning

- readiness: medium-high
- basis:
  - explicit requirements, providers, and transitive dependencies
  - target filtering
  - provider provenance
  - versioned shared-library filenames now match declared library requirements
- remaining limit:
  - `ResolvedLinkPlan` is a normalized planning artifact, not a final filesystem-resolved linker
    invocation

### Consumer Integration

- readiness: high for the documented narrow consumer profile
- basis:
  - contract fixtures
  - producer-side `fol` acceptance tests
  - explicit gating guidance

## How To Read This Scorecard

High means the subsystem is a reliable contract surface for normal downstream use.

Medium-high means the subsystem is suitable for serious use, but consumers should still respect the
documented limitations and keep policy checks explicit.

This scorecard should be revised whenever:

- platform scope changes
- consumer contract surfaces change
- new regressions widen or narrow the tested boundary
