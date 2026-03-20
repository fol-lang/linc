# LINC (link and binary evidence)

LINC is a Rust library for
link-surface analysis, native-symbol inspection, ABI probing, validation,
and binary evidence production.

It sits in the `PARC -> LINC -> GERC` pipeline:

- **PARC** (`pac`) handles C preprocessing, parsing, and declaration extraction
- **LINC** (`linc`) consumes normalized source contracts, inspects native artifacts,
  validates declarations against symbols, and produces link/binary evidence
- **GERC** (`gec`) consumes that evidence to emit Rust projections

## What LINC Produces

- `BindingPackage` — machine-readable binding metadata with declarations, macros,
  layouts, link surface, and diagnostics
- `SymbolInventory` — exported/imported symbols from ELF, Mach-O, COFF, and PE artifacts
- `ValidationReport` — declaration-vs-artifact match evidence
- `ResolvedLinkPlan` — normalized link plan with provider matching

## Usage

```rust
use linc::{from_source_package, SourcePackage, to_json};

// Build a source package from any frontend
let mut src = SourcePackage::default();
// ... populate declarations, macros, link requirements ...

// Convert to LINC's analysis package
let package = from_source_package(&src);

// Serialize for downstream tooling
let json = to_json(&package).unwrap();
```

For raw-header scanning (transitional, uses `parc` internally):

```rust
use linc::HeaderConfig;

let result = HeaderConfig::new()
    .header("mylib.h")
    .include_dir("/usr/local/include")
    .process()
    .unwrap();
```

For ABI-sensitive workflows:

1. Inspect `package.diagnostics`
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
