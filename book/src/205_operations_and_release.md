# Operations And Release

This section covers the operational and release posture of LINC.

## Operations

LINC is a library-first analysis tool. It is meant to be embedded, tested,
and serialized, not launched as a separate end-user service.

## Release

A release should be judged on build and test health, JSON contract stability,
documentation alignment, fixture coverage, and platform support posture.

The practical release split is:

- hermetic vendored baselines must stay green everywhere
- host-dependent large evidence ladders should stay green where available
- failure suites must keep proving conservative behavior instead of optimistic
  guessing

The grouped failure suites now live in:

- `failure_matrix_link` for unresolved and duplicate provider outcomes
- `failure_matrix_validation` for hidden, kind-mismatch, and ABI-mismatch
  validation states
- `failure_matrix_probe` for invalid bootstrap config and probe
  unavailable-vs-failed separation

The architectural rule remains the same here too:

- LINC owns evidence and analysis
- downstream build and generation policy still belongs outside LINC
- tests/examples/harnesses are where cross-package composition is proven
