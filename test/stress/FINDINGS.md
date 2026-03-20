# Stress Findings Ledger

This document is the rolling evidence ledger for the stress-plan examples.

`fol`-specific priority labels are defined separately in
[FOL_RELEVANCE.md](/home/bresilla/data/code/bresilla/bic/test/stress/FOL_RELEVANCE.md).

It is deliberately short and operational:

- what the example proved
- what limitation it exposed
- how urgent that limitation is for downstream consumers
- whether a follow-up fix has landed

## Current Top Findings

| Id | Area | Example | Finding | Status |
|---|---|---|---|---|
| `SF-001` | extraction | `c_interop_torture.h` | attribute-bearing packed typedef forms can block declaration extraction even when preprocessing and layout probes succeed | fixed with regression |
| `SF-005` | extraction | aligned torture typedef | attribute-bearing aligned typedef forms are the next parser-hostile declaration shape worth targeting after the packed typedef fix | open |
| `SF-002` | ABI confidence | opaque/incomplete probe subjects | a requested layout probe can currently fail the whole scan instead of degrading into retained diagnostics | fixed with regression |
| `SF-003` | link planning | real shared library inventories | versioned shared-library filenames are not always matched as providers for a declared library name | fixed with regression |
| `SF-006` | link planning | macOS-style provider inventories | text stub provider paths such as `libSystem.tbd` are realistic planning artifacts and should resolve like ordinary declared library providers | fixed with regression |
| `SF-004` | mixed-surface realism | combined daemon target | the mixed fixture is analyzable, but runtime subsystem availability still depends on downstream artifact inspection and deployment policy | observed |

## Example-by-Example Notes

### Synthetic Torture Header

- strong evidence retained today:
  - preprocessing
  - macro capture
  - requested layout probes
- current weak point:
  - parser-hostile aligned typedef declarations are still only inventoried, not yet recovered

### Linux System Headers

- strong evidence retained today:
  - code-driven configuration
  - layout probes for concrete system records
  - explicit Linux/libc metadata
- reproducibility classification:
  - `epoll` now has a repo-owned fixture fallback for its default analysis path
  - `linux_event_loop` remains host-header dependent
  - `socketcan` analysis remains host-header dependent
  - `socketcan` runtime smoke remains host-runtime dependent and opt-in

### Real-Library Ladder

- `zlib` is the clean baseline
- `libpcap` stresses callbacks and prerequisite system typedef visibility
- `libcurl` stresses macro and option surfaces
- `OpenSSL` stresses opaque-handle policy
- current provider-name refinement now covered:
  - Apple-style `.tbd` stub names resolve through the same declared-library matching path as other
    ordinary macOS provider inventories

### Plugin and Combined Daemon Surfaces

- strong evidence retained today:
  - ABI surface extraction
  - callback and opaque-handle modeling
  - explicit host-side `dl` dependency metadata
- current weak point:
  - runtime loader policy and deployment-time discovery remain downstream concerns

## Operating Rule

The goal of this ledger is not to accumulate complaints.
The goal is to drive the next implementation slices.

A finding should move from:

- `open`

to:

- `fixed with regression`
- `documented as downstream policy`
- or `accepted as non-blocking`
