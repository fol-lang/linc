# BIC (bind c)

`bic` is a Rust library for C header extraction, ABI/layout probing, native-symbol inspection,
validation, and normalized link-surface production on top of
[PAC](https://github.com/bresilla/pac).

It is intended to produce machine-readable binding metadata.
It is not a full native linker, a full platform loader simulation, or a standalone binary tool.

Today the strongest tested scope is:

- Linux and other ELF-oriented flows
- macOS / Mach-O inventory and validation evidence
- library-driven integration through `BindingPackage`, `ValidationReport`, and `ResolvedLinkPlan`
- stress-tested code-driven examples covering:
  - Linux system headers such as SocketCAN and epoll
  - real-library surfaces such as `zlib`, `libpcap`, `libcurl`, and `OpenSSL`
  - runtime-loaded plugin ABI boundaries
  - one combined daemon-style mixed surface

The current stress cycle also tightened three previously open limits:

- packed typedef records in preprocessed headers now recover into declaration extraction
- failed opaque/incomplete layout probes now degrade into retained diagnostics instead of aborting
  the scan
- declared library requirements can now match versioned shared-library filenames such as
  `libssl.so.3`

Consumers should treat `bic` as an evidence-producing library:

- declarations come from `BindingPackage`
- diagnostics are contractual data
- ABI/layout confidence comes from `layouts` and validation evidence
- native dependency intent comes from `package.link` and `ResolvedLinkPlan`

## Usage

```rust
use bic::{emit_rust_ffi, HeaderConfig};

let result = HeaderConfig::new()
    .header("mylib.h")
    .include_dir("/usr/local/include")
    .process()
    .unwrap();

let rust_ffi = emit_rust_ffi(&result.package);
println!("{}", rust_ffi);
```

For ABI-sensitive or native-link-aware workflows, the recommended next steps are:

1. inspect `result.package.diagnostics`
2. probe required layouts with `probe_type_layout(...)`
3. inspect artifacts with `inspect_symbols(...)`
4. validate declarations against artifacts with `validate(...)`
5. consume `package.link` or `resolve_link_plan(...)` downstream

## Building

```sh
make build
make test
```

The test suite is the primary statement of supported behavior.
If README wording and tests disagree, the tests are authoritative and the docs should be tightened.

## License

Dual-licensed under Apache 2.0 or MIT (see `LICENSE-APACHE` and `LICENSE-MIT`).
