# Getting Started

This chapter shows the shortest path from "I have parsed source metadata" to "I have
machine-readable link analysis".

LINC should be read as a library that produces analysis artifacts.
It should not be read as a promise that every successful scan is ready for generation without
additional policy checks.

## Add the Crate

Use a local path dependency while developing in the workspace:

```toml
[dependencies]
linc = { path = "../linc" }
```

If you need native artifact inspection and validation, enable the `symbols` feature.

Example:

```toml
[dependencies]
linc = { path = "../linc", features = ["codegen", "symbols"] }
```

## Preferred Contract-First Example

```rust
use linc::{analyze_source_package, SourceDeclaration, SourceFunction, SourcePackage, SourceType};

fn main() -> Result<(), String> {
    let mut source = SourcePackage::default();
    source.declarations.push(SourceDeclaration::Function(SourceFunction {
        name: "mylib_init".into(),
        parameters: vec![],
        return_type: SourceType::Int,
        variadic: false,
        source_offset: None,
    }));

    let analysis = analyze_source_package(&source);

    println!(
        "declared link inputs: {}",
        analysis.declared_link_surface.ordered_inputs.len()
    );
    println!(
        "has resolved plan: {}",
        analysis.resolved_link_plan.is_some()
    );

    Ok(())
}
```

The preferred output contract is `LinkAnalysisPackage`.
That is the value downstream generators should learn to consume.

## Transitional Raw-Header Scan

```rust
use linc::{analyze_source_package, HeaderConfig};

let result = HeaderConfig::new()
    .header("api.h")
    .include_dir("vendor/include")
    .library_dir("vendor/lib")
    .define("MYLIB_FEATURE_X", Some("1".into()))
    .link_lib("mylib")
    .link_shared_lib("dl")
    .probe_type_layout("struct api_context")
    .process()?;

let analysis = analyze_source_package(&linc::intake::adapters::from_binding_package(&result.package));
```

This path still exists so the repository can bootstrap itself from real headers, but it is not the
architectural target.

The long-term split is:

- `parc` owns source/header understanding
- `linc` owns link and binary evidence
- `gerc` consumes both in parallel

## JSON Round Trip

`LinkAnalysisPackage` is the contract intended to be exchanged across tools.

```rust
use linc::{analyze_source_package, LinkAnalysisPackage, SourcePackage};

let analysis = analyze_source_package(&SourcePackage::default());

let json = serde_json::to_string_pretty(&analysis).unwrap();
let restored: LinkAnalysisPackage = serde_json::from_str(&json).unwrap();

assert_eq!(analysis, restored);
```

## Common Integration Pattern

The most common downstream pattern is:

1. Produce a `SourcePackage` in `parc` or another frontend
2. Call `analyze_source_package`
3. Optionally inspect artifacts with `inspect_symbols`
4. Optionally validate against those artifacts
5. Feed `SourcePackage` plus `LinkAnalysisPackage` into your generator/build system

## First Things To Inspect

When an analysis result does not look right, inspect these fields first:

- `analysis.declared_link_surface`
- `analysis.resolved_link_plan`
- `analysis.diagnostics`
- `analysis.abi_probe`
- `analysis.validation`
- `analysis.symbol_inventories`

Those surfaces usually tell you whether the problem is:

- source intake adaptation
- ABI probing
- link metadata declaration
- provider discovery
- validation

## Library-Only Design

LINC is intended to be consumed as a Rust library.

That means the normal integration path is:

1. call `analyze_source_package()` or other library APIs directly
2. serialize the resulting values if another tool needs JSON
3. keep executable/tooling policy in the downstream crate rather than in LINC itself
