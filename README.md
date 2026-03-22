# LINC

LINC is the link and binary evidence layer in the `parc -> linc -> gerc`
toolchain.

It owns evidence: declared native inputs, discovered artifacts, resolved link
plans, ABI probe results, and validation findings.

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
- resolve provider choices into `ResolvedLinkPlan`
- probe ABI-sensitive layouts
- validate declarations against observed native artifacts
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

## Hardening Matrix

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

## Release Gates

`linc` should only be treated as release-ready when all of these remain green:

- `make build`
- `make test`
- unit and artifact-boundary suites
- hermetic vendored examples
- validation/link-plan edge suites
- typed operational-error matrix
- ELF/Mach-O/Windows confidence-floor matrix
- Mach-O framework and dylib provider-policy matrix
- explicit runtime-loader boundary surfaces
- at least one OpenSSL-style host-dependent evidence target
- at least one combined Linux/system evidence target
- for whole-pipeline production claims, confirm the current downstream `gerc`
  canonical anchors still ingest `linc` evidence cleanly in tests/examples

The current canonical evidence surfaces are:

- vendored zlib
- vendored libpng
- plugin ABI
- combined daemon fixture
- OpenSSL
- Linux event-loop stack

## Canonical Corpus

The current LINC production corpus is intentionally named:

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

Those are the evidence surfaces LINC should be judged against first.

Whole-pipeline readiness is stricter than crate-local readiness: downstream
`gerc` canonical anchors are part of the final production bar even though
`linc/src/**` must not depend on `gerc`.

## Build And Test

```sh
make build
make test
```

## License

Dual-licensed under Apache 2.0 or MIT.
