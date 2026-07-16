# Hardening Evidence Scorecard

This chapter summarizes current release readiness by subsystem and ties the
score directly to the current hardening ladder.

## Overall Evidence

LINC is in H0 hardening and is not production-certified. Hermetic and required
Linux system tests provide useful evidence, while the largest native-library
fixtures remain prerequisite-dependent. Mach-O and Windows coverage is
controlled/synthetic and has no native CI gate. Link plans are not filesystem
resolution, and symbol matches are not full ABI validation.

## Subsystem Scorecard

- source-shaped intake: version-1 fixture-backed behavior
- JSON artifacts: roundtrip evidence, not the frozen H1 schema
- layout evidence: partial compiler/target observations
- ELF inventories: hermetic and Linux system evidence
- Mach-O/Windows inventories: synthetic fixture evidence only
- validation: symbol/kind plus optional shape observations, not ABI proof
- link planning: normalized candidate association, not filesystem resolution
- consumer integration: test/example evidence, not H5 certification

## Canonical Readiness Anchors

The regression baseline should be checked against these anchors first:

- vendored zlib
- vendored libpng
- plugin ABI fixture
- combined daemon fixture
- difficult-record evidence fixtures
- OpenSSL when available
- Linux event-loop analysis when available

If those anchors drift, confidence in the current baseline should drop even if
many smaller unit tests still pass. Green anchors do not complete H1-H5.

## How To Read This Scorecard

The labels above describe kinds of evidence rather than numerical readiness.
Consumers must inspect target identity, provider state, validation phases, and
optional shape evidence explicitly.
