# Future Release Checklist

Registry publication and version changes are deferred to H6. Use this checklist
only after the contract and certification milestones are complete; it does not
describe current release readiness.

## Build And Test

- run `make build`
- run `make fmt-check`
- run `make lint`
- run `make check-features`
- run `make test`
- run `make test-contract`
- run `make test-package`
- run the required `make test-system` lane with all prerequisites installed
- run `make docs-check`
- run `make verify` from a clean worktree

## Canonical Hardening Gates

- confirm hermetic baselines still pass
  - vendored zlib
  - vendored libpng
  - plugin ABI
  - combined daemon fixture
- confirm at least one host-dependent large-evidence ladder still passes where
  available
  - OpenSSL
  - Linux event-loop stack
- confirm failure suites still reject duplicate, unresolved, hidden, decorated,
  and ABI-questionable cases conservatively
- confirm plugin-style `dl` surfaces still produce explicit runtime-boundary
  notes instead of over-claiming runtime truth
- confirm the hermetic ELF static and synthetic Mach-O/Windows fixture suite
  still passes without presenting synthetic coverage as native certification
- confirm determinism anchors still hold on the canonical large surfaces

## Contract Surfaces

- confirm the documented JSON artifact shapes remain consumable by the current
  schema version
- confirm `ValidationReport` fixture coverage still matches current structured
  fields

## Documentation

- confirm README wording matches tested behavior
- confirm the book reflects current API entry points and platform scope

## Consumer Boundary

- confirm the generic library contract stays primary
- confirm cross-package composition is still described as tests/examples/
  harness work, not crate-to-crate library coupling

## Release Decision

Do not treat "builds successfully" as sufficient. The code, docs, and fixtures
all need to match the same boundary. H6 also requires a fresh package/legal/
publication audit; H0 gates alone are insufficient.
