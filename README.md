# LINC (link and binary evidence)

LINC is the link-analysis layer of the pipeline. It takes source-shaped input,
normalizes declared native dependencies, inspects real artifacts, validates
source claims against binary reality, and emits machine-readable evidence.

In the intended architecture:

- `parc` owns source meaning
- `linc` owns link and binary meaning
- `gec` owns Rust lowering and emitted build metadata

Those roles are intentionally separate. `linc` is not a parser, not a header
driver, and not a Rust generator.

## Architectural Rules

`linc` owns its own internal model and its own evidence artifacts.

- `linc/src/**` must not depend on `parc` or `gec`
- cross-package translation belongs only in tests, examples, or external harnesses
- there is no shared ABI crate
- there is no backward-compatibility burden for old pipeline shapes
- repo-local bootstrap utilities are allowed to exist, but they are not the public architecture

## Responsibilities

- consuming source-shaped declarations and declared link intent
- inspecting native artifacts for symbol evidence
- probing ABI-relevant layout information
- validating declarations against binary reality
- resolving normalized native link requirements
- emitting evidence artifacts for downstream tools

## Non-responsibilities

- parsing or preprocessing C as the public architecture
- owning a universal pipeline envelope
- Rust lowering or code generation
- downstream runtime or loader policy

The practical consequence is simple:

1. some frontend emits a source artifact
2. a test, example, or harness translates that artifact into `linc` input
3. `linc` emits evidence artifacts
4. a downstream generator consumes those artifacts on its own terms

## What LINC Produces

The main output families are:

- `LinkAnalysisPackage`
  a normalized evidence bundle derived from source intent plus optional binary inspection
- `SymbolInventory`
  exported/imported symbol evidence from ELF, Mach-O, COFF, and PE artifacts
- `ValidationReport`
  declaration-vs-artifact evidence, including missing and mismatched cases
- `ResolvedLinkPlan`
  normalized library/framework/object requirements with provider matching

These are evidence products, not parser products.

## Core Workflow

```rust
use linc::{analyze_source_package, SourcePackage};

let mut src = SourcePackage::default();
// populate declarations, macros, and declared native link requirements

let analysis = analyze_source_package(&src);
let json = serde_json::to_string_pretty(&analysis).unwrap();
```

For more serious native pipelines, the usual sequence is:

1. analyze declared link surface with `analyze_source_package(...)`
2. inspect artifacts with `inspect_symbols(...)`
3. validate source intent with `validate(...)` or `validate_many(...)`
4. resolve concrete provider choices with `resolve_link_plan(...)`
5. pass source artifacts and evidence artifacts to the downstream tool

## Artifact Boundary

The real integration boundary is serialized artifacts, not shared Rust types
across crates.

- `parc` may serialize a source artifact
- tests/examples/harnesses may translate that artifact into `linc` input
- `linc` may serialize `LinkAnalysisPackage`, `SymbolInventory`, or `ValidationReport`
- `gec` or another consumer may load those artifacts through its own test/example code

Library code inside `linc` must not be the place where cross-package translation lives.

## Tested Scope

The suite currently exercises:

- Linux and other ELF-oriented flows
- macOS / Mach-O inventory and validation evidence
- split-pipeline artifact tests using `parc` fixtures
- difficult header surfaces including zlib, libpcap, libcurl, OpenSSL, SocketCAN, epoll, and libpng

The tests are the main statement of supported behavior.

## Build And Test

```sh
make build
make test
```

## License

Dual-licensed under Apache 2.0 or MIT (see `LICENSE-APACHE` and `LICENSE-MIT`).
