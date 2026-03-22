# Release Checklist

Use this checklist before cutting a release candidate.

## Build And Test

- run `make build`
- run `make test`

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
- confirm the hermetic ELF static, Mach-O framework, and Windows PE fixture
  suite still passes
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
all need to match the same boundary.
