# BIC Reference

`bic` is the machine-facing C interop analysis layer in the Bresilla toolchain.

Its job is not to compile C code and it is not a full linker on its own.
Its job is to take C-facing inputs such as headers, preprocessed translation units, and native artifacts, then normalize that information into a form downstream tooling can trust.

In practice that means `bic` sits between:

- `pac`, which handles preprocessing and parsing
- native artifacts such as `.o`, `.a`, `.so`, and `.dylib`
- downstream consumers such as `fol`, code generators, or validation/reporting tooling

## What BIC Produces

The core output is a `BindingPackage`.

That package is intentionally machine-oriented. It contains:

- binding items extracted from headers
- target/compiler metadata for the scan
- declared and normalized native link inputs
- captured macro inventory
- compiler-probed type layouts
- diagnostics produced during extraction

When native artifacts are involved, `bic` can also produce:

- `SymbolInventory` values from `inspect-symbols`
- `ValidationReport` values from `validate`

## Mental Model

The practical pipeline looks like this:

```text
headers / preprocessed C / native artifacts
    -> preprocessing and parsing
    -> declaration extraction
    -> macro capture
    -> optional ABI layout probing
    -> link surface normalization
    -> optional artifact inspection
    -> optional declaration-vs-artifact validation
```

Another way to say it:

- `pac` tells `bic` what the C source says
- the compiler helps `bic` discover ABI facts
- artifact inspection tells `bic` what native binaries actually export
- `bic` packages the result into JSON-friendly structures

## What BIC Is Good At

Today `bic` is especially useful for:

- extracting C declarations from real headers
- filtering out irrelevant system-header noise
- producing stable JSON for other tools
- generating Rust FFI stubs from extracted declarations
- inventorying exported symbols from ELF and Mach-O artifacts
- comparing declarations against one or more native artifacts
- preserving native link metadata alongside the extracted API surface

## What BIC Does Not Try To Be

`bic` is not:

- a C compiler
- a full semantic C type checker
- a full platform linker driver
- a replacement for build-system-native concepts such as rpaths, linker scripts, or loader policy

That separation matters. The intended division of labor is:

- `pac` parses
- `bic` analyzes and normalizes
- `fol` or another consumer decides how to generate, compile, and link final output

## Main Public Surfaces

Most users touch one or more of these library entry points:

- `HeaderConfig` for scanning raw headers
- `PreprocessedInput` for parsing already-preprocessed source
- `probe_type_layouts` for compiler-assisted ABI layout probing
- `inspect_symbols` for reading native artifact symbols
- `validate` and `validate_many` for declaration-vs-artifact checks
- `emit_rust_ffi` for Rust FFI emission when the `codegen` feature is enabled

## Recommended Reading Order

The book is structured in a few bigger blocks now:

1. Getting Started and the core extraction chapters
2. Native Evidence
3. API Contract and the contract/policy chapters
4. End-To-End Workflows
5. Operations And Release

If you only want to integrate `bic` into another tool, focus on:

- [Header Processing](./020_headers.md)
- [IR Model](./030_ir.md)
- [Native Evidence](./095_native_evidence.md)
- [API Contract](./100_api_contract.md)
- [Contracts And Policy](./115_contracts_and_policy.md)
- [End-To-End Workflows](./110_workflows.md)
