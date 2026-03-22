# Readiness Scorecard

This chapter summarizes current release readiness by subsystem and ties the
score directly to the current hardening ladder.

## Overall Readiness

LINC should currently be read as:

- strong on hermetic evidence production
- strong on ELF-first symbol and validation workflows
- useful but more conservative on Mach-O and Windows import-library paths
- meaningfully hardened on vendored and daemon-style fixtures
- still dependent on host availability for the largest OpenSSL and Linux-system
  ladders

For whole-pipeline claims, this score is also capped by downstream `gerc`
anchors that ingest `linc` evidence in tests/examples.

## Subsystem Scorecard

- source-shaped intake: high
- JSON artifact stability: high
- ABI layout evidence: medium-high
- symbol inventories: high for ELF, medium-high for Mach-O, medium for Windows
- validation: medium-high
- link planning: medium-high
- hermetic large-surface confidence: high
- host-dependent large-surface confidence: medium-high
- consumer integration on the documented artifact boundary: high

## Canonical Readiness Anchors

The release posture should be judged against these anchors first:

- vendored zlib
- vendored libpng
- plugin ABI fixture
- combined daemon fixture
- OpenSSL when available
- Linux event-loop analysis when available

If those anchors drift, the scorecard should drop even if many smaller unit
tests still pass.

## How To Read This Scorecard

High means the subsystem is a reliable contract surface for normal downstream
use. Medium-high means consumers should still respect the documented limits and
expect some host/platform asymmetry. Medium means the subsystem is useful but
should not be oversold as equally mature across all supported environments.
