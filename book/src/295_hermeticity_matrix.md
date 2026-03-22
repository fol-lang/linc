# Hermeticity Matrix

This chapter turns the large LINC evidence suite into an explicit hermeticity
ladder.

The central question is not just "does a test pass". The central question is
"what kind of evidence confidence does this surface buy us".

## Tier 1: Always-On Hermetic Baselines

These are the first confidence anchors and should remain green everywhere:

- vendored zlib
- vendored libpng
- plugin ABI fixtures
- combined daemon and max-pain fixtures

These surfaces prove that LINC can:

- consume source-shaped input
- derive declared link surface
- resolve providers on controlled artifacts
- emit stable evidence and validation products

## Tier 2: Host-Dependent High-Value Ladders

These add confidence on real native environments when the libraries and headers
exist:

- OpenSSL
- Linux event-loop stack
- epoll and socketcan examples
- other real system-library probes in the stress suites

These surfaces matter because they are closer to the real deployment problem
than vendored toy cases.

## Tier 3: Failure And Conservative-Evidence Surfaces

These prove that LINC is refusing or degrading honestly:

- duplicate provider cases
- unresolved provider cases
- hidden or decorated symbol mismatches
- ABI-questionable fixtures
- partial or missing layout evidence

Those are release-positive tests when they stay:

- deterministic
- diagnostic
- intentionally conservative

## Determinism Anchors

The most important repeat-run anchors right now are:

- vendored zlib
- vendored libpng
- combined daemon fixture
- OpenSSL when available
- Linux event-loop analysis

If any of those become unstable, the evidence story should be treated as
weaker, even if many unit tests still pass.
