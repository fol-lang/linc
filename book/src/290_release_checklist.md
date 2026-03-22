# Release Checklist

This chapter is the final release-readiness checklist for LINC.

It is intentionally operational.
Use it before cutting a release candidate or publishing a version that downstream consumers are
expected to pin.

## Build And Test

- run `make build`
- run `make test`
- confirm the normal library test suite passes without newly introduced ignores
- confirm producer-side acceptance tests still pass
- confirm regression fixtures still deserialize and round-trip where expected

## Contract Surfaces

- confirm `BindingPackage` JSON remains consumable by the documented schema version
- confirm `ValidationReport` fixture coverage still matches current structured fields
- confirm `ResolvedLinkPlan` remains consumable through the documented downstream subset
- confirm new additive fields use `#[serde(default)]` only when the documented artifact shape needs it

## Documentation

- confirm README wording matches tested behavior and does not overclaim
- confirm the book reflects current API entry points and current platform scope
- confirm documented guarantees are backed by tests or explicit fixtures
- confirm limitations are still described where behavior is conservative rather than complete

## ABI And Native Evidence

- confirm ABI-sensitive validation changes are covered by regression fixtures
- confirm layout-backed flows remain tested through producer-side acceptance coverage
- confirm ELF and Mach-O inventories still match the intended conservative provider policy
- confirm symbol-provider ambiguity and unresolved-provider paths are still surfaced explicitly

## Consumer Boundary

- confirm the generic library contract stays primary
- confirm `fol` guidance is still documented as consumer guidance, not universal crate policy
- confirm the producer-side `fol` acceptance tests still model the intended narrow consumer subset

## Release Decision

Do not treat "builds successfully" as sufficient.

LINC is ready to release only when:

- the code builds
- the tests pass
- the documented contract still matches the code
- the contract fixtures still match the intended consumer boundary
- the current platform/support claims are still true
