# Support Tiers

This chapter groups current test evidence. These are not production support
tiers or semver guarantees.

## Tier Definitions

- Hermetic: required repository-owned fixtures
- System: required prerequisite-dependent Linux tests
- Synthetic: controlled format/platform fixtures without a native CI host

## Current Tier Assignment

ELF-oriented flows have the strongest current evidence. Mach-O and Windows
format behavior is exercised by controlled fixtures, but native Apple and
Windows certification is absent in H0.

## Downstream Guidance

Consumers must treat all three as pre-certification evidence and must not infer
uniform provider, linker, loader, or ABI behavior across platforms.
