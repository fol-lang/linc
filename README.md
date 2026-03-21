# LINC (link and binary evidence)

LINC is a Rust library for link-surface analysis, native-symbol inspection,
ABI probing, validation, and binary evidence production.

It sits in the `PARC -> LINC -> GERC` pipeline:

- **PARC** (`parc`) handles C preprocessing, parsing, and declaration extraction
- **LINC** (`linc`) consumes normalized source contracts, inspects native artifacts,
  validates declarations against symbols, and produces link/binary evidence
- **GERC** (`gec`) consumes that evidence to emit Rust projections

Architecturally, `linc` owns its own model and artifact.

- `linc` library code must not depend on `parc` or `gec`
- `linc` may use `parc` only in tests/examples or external harnesses
- there is no shared ABI crate
- if `linc` consumes `parc` output, that translation belongs outside `linc/src/**`

## What LINC Produces

- `LinkAnalysisPackage` — machine-readable link and binary evidence derived from a source contract
- `SymbolInventory` — exported/imported symbols from ELF, Mach-O, COFF, and PE artifacts
- `ValidationReport` — declaration-vs-artifact match evidence
- `ResolvedLinkPlan` — normalized link plan with provider matching

## Usage

```rust
use linc::{analyze_source_package, SourcePackage};

// Build a source package from any frontend
let mut src = SourcePackage::default();
// ... populate declarations, macros, link requirements ...

// Convert to LINC's analysis package
let analysis = analyze_source_package(&src);

// Serialize for downstream tooling
let json = serde_json::to_string_pretty(&analysis).unwrap();
```

Raw-header scanning still exists as a repo-local bootstrap path, but it is not
the long-term architecture. The preferred story is:

1. `parc` or another frontend emits a source artifact
2. tests/examples or an external harness translate that artifact into `linc` input
3. `linc` emits a `LinkAnalysisPackage` or other evidence artifacts

For ABI-sensitive workflows:

1. Inspect `analysis.diagnostics`
2. Probe layouts with `probe_type_layouts(...)`
3. Inspect artifacts with `inspect_symbols(...)`
4. Validate with `validate(...)`
5. Construct link plans with `resolve_link_plan(...)`

## Tested Scope

- Linux and other ELF-oriented flows
- macOS / Mach-O inventory and validation evidence
- Stress-tested against: zlib, libpcap, libcurl, OpenSSL, SocketCAN, epoll

## Building

```sh
make build
make test
```

The test suite is the primary statement of supported behavior.

## License

Dual-licensed under Apache 2.0 or MIT (see `LICENSE-APACHE` and `LICENSE-MIT`).
