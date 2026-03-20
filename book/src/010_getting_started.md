# Getting Started

This chapter shows the shortest path from "I have a header" to "I have machine-readable binding
metadata".

LINC should be read as a library that produces analysis artifacts.
It should not be read as a promise that every successful scan is ready for generation without
additional policy checks.

## Add the Crate

Use a local path dependency while developing in the workspace:

```toml
[dependencies]
bic = { path = "../bic" }
```

If you need Rust FFI generation, enable the `codegen` feature.
If you need native artifact inspection and validation, enable the `symbols` feature.

Example:

```toml
[dependencies]
bic = { path = "../bic", features = ["codegen", "symbols"] }
```

## Smallest Useful Example

```rust
use bic::HeaderConfig;

fn main() -> Result<(), String> {
    let result = HeaderConfig::new()
        .header("mylib.h")
        .process()?;

    println!("items: {}", result.package.items.len());
    println!("macros: {}", result.package.macros.len());
    println!("layouts: {}", result.package.layouts.len());

    Ok(())
}
```

`process()` returns a `RawHeaderResult` with two parts:

- `package`: the durable output you usually keep or serialize
- `report`: the immediate preprocessing invocation details and preprocessed source

## Typical Scan With Real Inputs

Most real scans need include paths, defines, and some native link metadata:

```rust
use bic::HeaderConfig;

let result = HeaderConfig::new()
    .header("api.h")
    .include_dir("vendor/include")
    .library_dir("vendor/lib")
    .define("MYLIB_FEATURE_X", Some("1".into()))
    .link_lib("mylib")
    .link_shared_lib("dl")
    .probe_type_layout("struct api_context")
    .process()?;
```

That single scan can now carry:

- extracted declarations
- captured preprocessor macros
- link requirements
- probed type layouts
- diagnostics

Treat those as evidence surfaces.
Downstream generators should still decide which diagnostics, layouts, and validation findings are
required before generation is allowed.

## JSON Round Trip

`BindingPackage` is designed to be exchanged across tools.

```rust
use bic::{from_json, to_json, HeaderConfig};

let result = HeaderConfig::new()
    .header("mylib.h")
    .process()?;

let json = to_json(&result.package).unwrap();
let restored = from_json(&json).unwrap();

assert_eq!(result.package, restored);
```

This is the normal handoff point to another system.

## Preprocessed Input Path

Sometimes you already have a `.i` file or a preprocessor pipeline elsewhere.
In that case, skip raw-header driving and feed the preprocessed text directly.

```rust
use bic::PreprocessedInput;

let pkg = PreprocessedInput::from_string("int add(int a, int b);")
    .with_path("generated.i")
    .extract();

assert_eq!(pkg.items.len(), 1);
```

Use this mode when:

- another build system already owns preprocessing
- you want fully reproducible parser input checked into tests
- you need to debug extraction separate from compiler invocation

## Common Integration Pattern

The most common downstream pattern is:

1. Run `HeaderConfig::process()`
2. Serialize the `BindingPackage`
3. Optionally inspect artifacts with `inspect_symbols`
4. Optionally validate the package against those artifacts
5. Feed the package and validation results into your generator/build system

That is the intended shape for `fol` integration as well, but it is also the general recommended
shape for other consumers that want stable machine contracts instead of ad hoc extraction.

## First Things To Inspect

When a scan does not look right, inspect these fields first:

- `package.items`
- `package.macros`
- `package.layouts`
- `package.link`
- `package.diagnostics`
- `report.preprocessed_source`

Those six surfaces usually tell you whether the problem is:

- preprocessing
- extraction
- macro visibility
- ABI probing
- link metadata declaration

## Library-Only Design

LINC is intended to be consumed as a Rust library.

That means the normal integration path is:

1. call `HeaderConfig::process()` or other library APIs directly
2. serialize the resulting values if another tool needs JSON
3. keep executable/tooling policy in the downstream crate rather than in LINC itself
