# LINC

LINC is the link and binary evidence layer in the `parc -> linc -> gerc`
toolchain.

It owns evidence: declared native inputs, discovered artifacts, resolved link
plans, ABI probe results, and validation findings.

## Hardening Status

LINC is being hardened as the native-evidence and provider-resolution owner for
the sibling PARC/LINC/GERC pipeline. It is not yet certified for FOL V4.

The distribution package is `follang-linc`; the Rust library name remains
`linc`. Registry publication is deferred until the H6 distribution gate, and
the crate version remains unchanged during baseline hardening. The declared
minimum supported Rust version (MSRV) is Rust 1.89.

## Current Support Boundary

| Area | Current evidence | Boundary |
|---|---|---|
| Link requirements and plans | Normalization and controlled-inventory tests | `ResolvedLinkPlan` is not a filesystem-resolved linker invocation. A `Resolved` requirement only means one candidate inventory matched the declared artifact or filename shape. |
| Symbol validation | Symbol name, direction, visibility, kind, and optional shape evidence | A `Matched` symbol is not by itself ABI validation. Full calling convention, aggregate passing, variadics, target ABI, linkability, and runtime loading are not proved. |
| Layout probing | Compiler-probe reports and fixture coverage | Evidence may be partial and is tied to the compiler/target that produced it. |
| Linux / ELF | Host and hermetic fixtures exist | The strongest current evidence path, but not yet a certified production tier. |
| Apple / Mach-O and Windows / COFF | Parser/inventory and synthetic fixtures exist | Neither platform is certified; H0 has no native Apple or Windows CI gate. |
| Serialized packages | Schema version 1 artifacts are exercised | Current schemas and permissive/defaulted fields are a hardening baseline, not the frozen H1 contract. |

## What LINC Actually Exposes Today

There are two real consumer layers in the crate:

1. a preferred contract-first layer centered on `SourcePackage` and
   `LinkAnalysisPackage`
2. a still-public lower-level layer centered on `BindingPackage`,
   `linc::ir::*`, and the repo-local `raw_headers` bootstrap path

The docs should not pretend the second layer is gone. It is still public and
still exercised by tests.

## Responsibilities

- consume source-shaped input through `SourcePackage`
- analyze declared link requirements
- inspect native artifacts for symbol evidence
- associate declared requirements with candidate inventories in `ResolvedLinkPlan`
- probe ABI-sensitive layouts
- report symbol and optional ABI-shape evidence from observed native artifacts
- serialize evidence products

## Non-Responsibilities

- owning source parsing/preprocessing as the main public boundary
- Rust code generation
- downstream crate-specific build policy
- library-level dependency on `parc` or `gerc`

## Preferred Surface

The preferred modern entrypoints are:

- `analyze_source_package`
- `LinkAnalysisPackage`
- `inspect_symbols`
- `probe_type_layouts`
- `validate` / `validate_many`

## Still-Public Lower-Level Surface

The crate root also still exposes:

- `BindingPackage` and related IR under `linc::ir`
- `raw_headers::HeaderConfig` and raw-header bootstrap helpers
- a large set of symbol/probe/validation/support types

That low-level surface is real. It is not the first story new consumers should
build around, but it is part of what the crate currently is.

## Minimal Contract-First Example

```rust
use linc::{analyze_source_package, SourcePackage};

let source = SourcePackage::default();
let analysis = analyze_source_package(&source);
println!("{}", analysis.declared_link_surface.ordered_inputs.len());
```

## Artifact Boundary

Cross-package composition belongs in tests, examples, and external harnesses.
`linc/src/**` stays self-contained even though `linc` may read and write
serialized artifacts that other tools also understand.

## Tested Scope

The current suite covers:

- source-contract analysis
- symbol inspection on ELF, Mach-O, and COFF-like inputs
- ABI probe reports
- validation reports
- explicit link, validation, and probe failure matrices
- raw-header bootstrap flows
- artifact-boundary tests using upstream fixtures
- large hostile/library surfaces such as zlib, libpng, libcurl, OpenSSL, and epoll

## Current Test Evidence

The current hardening ladder is easiest to read in four buckets:

- hermetic vendored baselines
  - zlib
  - libpng
  - plugin ABI
  - combined daemon fixture
- host-dependent evidence ladders
  - OpenSSL
  - Linux event-loop stack
  - epoll and socketcan system examples
- failure and validation surfaces
  - duplicate providers
  - unresolved providers
  - hidden or decorated symbol mismatches
  - ABI-questionable fixtures and partial layout evidence
  - explicit runtime-loader boundary notes on plugin-style surfaces
  - explicit link, validation, and probe failure matrices
  - explicit typed operational-error matrix
  - explicit ELF/Mach-O/Windows confidence-floor matrix
  - explicit Mach-O provider-policy matrix
  - expanded hermetic ELF/Mach-O/Windows artifact fixtures
- determinism anchors
  - zlib
  - libpng
  - OpenSSL when available
  - combined daemon fixture
  - Linux event-loop analysis

Those are the confidence anchors LINC should be judged against first.

Host-dependent evidence includes:

- OpenSSL
- Linux event-loop stack
- epoll and socketcan examples

The current canonical evidence surfaces are:

- vendored zlib
- vendored libpng
- plugin ABI
- combined daemon fixture
- OpenSSL
- Linux event-loop stack

The current LINC test corpus is intentionally named:

- hermetic vendored
  - zlib
  - libpng
  - plugin ABI
  - combined daemon fixture
- host-dependent raises
  - OpenSSL
  - Linux event-loop stack
  - epoll and socketcan examples
- conservative-failure anchors
  - typed operational-error matrix
  - duplicate-provider fixtures
  - unresolved-provider fixtures
  - ABI-questionable validation fixtures
  - partial-layout and packed-bitfield fixtures

Those are test anchors, not ABI certification, provider truth, or platform
certification. H1 through H5 of the hardening plan remain future milestones.

## Verification

```sh
make build
make fmt-check
make lint
make check-features
make test
make test-contract
make test-package
make test-system
make docs-check
make verify
```

`make test` is the hermetic required lane. `make test-system` runs the ignored,
prerequisite-dependent system tests as a required lane; a missing compiler,
native inspection tool, development header, or library required by a selected
test is a failure rather than a silent skip. Required CI installs those
prerequisites. `make docs-check` requires `mdbook` and builds both the book and
Rust API documentation without staging or committing output.

`make verify` expects a clean worktree, runs the common gates above, and proves
that validation did not change Git state. During local review of an already
dirty tree, `VERIFY_ALLOW_DIRTY=1 make verify` retains the before/after check.

## License

Dual-licensed under Apache 2.0 or MIT.
