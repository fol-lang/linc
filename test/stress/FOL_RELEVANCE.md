# `fol` Relevance Rubric

This document classifies stress findings by how much they matter to `fol`.

The library contract stays generic.
This rubric is only a consumer-priority lens layered on top of the generic findings.

## Labels

### Blocking

Use `blocking` when the issue prevents `fol` from safely doing one of these:

- extracting the intended declaration surface
- obtaining the ABI evidence needed for generation
- validating a required native provider path
- producing a correct downstream native dependency plan

### Non-Blocking

Use `non-blocking` when:

- the issue is real but there is a clean downstream workaround
- the issue affects optional fidelity rather than core correctness
- the issue affects a stress target that is not part of `fol`'s required baseline

### Future

Use `future` when:

- the issue is worth tracking
- but it does not currently stop `fol` from binding the target safely enough

## How To Apply The Rubric

For each stress target, ask four questions:

1. Can `fol` still obtain the declarations it needs?
2. Can `fol` still obtain enough ABI evidence to make generation decisions?
3. Can `fol` still distinguish native-link metadata from runtime-loader policy?
4. Is the remaining gap generic library work or `fol`-specific policy work?

If the answer to questions 1 or 2 is no, the finding is usually `blocking`.

If the answer to question 3 is no but the target is runtime-loaded by design, the finding is often
`non-blocking` because it belongs to downstream policy rather than `linc` pretending to be a full
loader simulator.

## SocketCAN Classification

### `blocking`

- none currently observed in the SocketCAN example

### `non-blocking`

- host-path discovery for Linux headers
  - reason: `fol` can still construct the scan in code once the deployment/toolchain contract is
    known, and the remaining issue is environment reproducibility rather than a missing analysis
    surface
- runtime success of `socket(AF_CAN, SOCK_RAW, CAN_RAW)`
  - reason: this is a deployment/runtime fact, not a missing header-analysis capability

### `future`

- a hermetic SocketCAN-oriented Linux-header fixture path
  - reason: useful for reproducibility, but not required for `fol` to consume the current
    code-driven analysis surface or the explicit runtime-boundary split

## Real-Library Ladder Classification

### `blocking`

- none currently observed in the `zlib` baseline or the broader library ladder after the first
  stress-cycle fixes landed

### `non-blocking`

- `libpcap` prerequisite typedef sensitivity
  - reason: this is mostly an environment/header-composition concern rather than a missing core
    extraction surface, and the main remaining gap is still host-package reproducibility
- `libcurl` macro-surface selectivity
  - reason: `fol` can still consume the declarations and the retained macro set without assuming
    that every option macro is a first-class bindable constant
- `OpenSSL` opaque-handle policy
  - reason: the correct downstream behavior is to preserve opacity, not to force fake layout
    certainty, and the current evidence model already supports that posture

### `future`

- deeper hermetic fixtures for the real-library ladder
  - reason: useful for stronger reproducibility and CI portability, but not currently stopping
    `fol` from consuming the existing library-first evidence surfaces

## Combined Daemon Classification

### `blocking`

- none currently observed in the combined daemon fixture after the deeper daemon-core evidence path
  landed

### `non-blocking`

- mixed subsystem availability remains downstream policy
  - reason: the combined target now has a concrete daemon-core validation path, but that still does
    not turn optional SocketCAN, packet capture, TLS, and plugin availability into a library-level
    guarantee
- explicit `dl` metadata is only host/runtime-loader intent
  - reason: `fol` still needs its own runtime-loader and packaging policy, and `linc` should not
    pretend otherwise

### `future`

- end-to-end combined native bundle validation
  - reason: valuable for deeper confidence beyond the daemon-core fixture, but not required for
    `fol` to consume the current combined analysis surface and apply its own deployment policy
