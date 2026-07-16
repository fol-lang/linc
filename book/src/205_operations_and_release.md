# Operations And Release

This section covers the operational and release posture of LINC.

## Operations

LINC is a library-first analysis tool. It is meant to be embedded, tested,
and serialized, not launched as a separate end-user service.

## Release

Registry publication and version changes are deferred to H6. H0 validation is
a feedback baseline, not evidence that LINC is release-ready or that the
version-1 JSON shapes are the frozen H1 contract.

The current test-evidence split is:

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

The common H0 gate is `make build`, `make fmt-check`, `make lint`,
`make check-features`, `make test`, `make test-contract`, `make test-package`,
`make test-system`, `make docs-check`, and `make verify`. The system lane is
required: missing compiler/tool/header/library prerequisites fail. Passing the
gate does not complete H1-H5.
