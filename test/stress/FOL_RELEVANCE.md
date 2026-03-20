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
`non-blocking` because it belongs to downstream policy rather than `bic` pretending to be a full
loader simulator.
