# End-To-End Workflows

This chapter ties the separate surfaces together into practical workflows.

## Workflow 1: Analyze A Source Contract And Save JSON

```rust
use linc::{analyze_source_package, LinkAnalysisPackage, SourcePackage};

let analysis = analyze_source_package(&SourcePackage::default());
let json = serde_json::to_string_pretty(&analysis).unwrap();
std::fs::write("link-analysis.json", json).unwrap();
```

This is the baseline path once a frontend already exists.

The resulting file now contains:

- declared link metadata
- resolved link-plan shape
- diagnostics
- optional probe/validation attachment points
- target and input provenance

## Workflow 2: Transitional Raw-Header Bootstrap

```rust
use linc::{analyze_source_package, HeaderConfig};

let result = HeaderConfig::new()
    .header("include/demo.h")
    .include_dir("include")
    .process()?;

let source = linc::intake::adapters::from_binding_package(&result.package);
let analysis = analyze_source_package(&source);
```

This still exists so the repository can bootstrap itself from real headers while
`parc` finishes taking over frontend ownership.

## Workflow 3: Inspect A Native Artifact

```rust
use linc::inspect_symbols;

let inventory = inspect_symbols("build/libdemo.so")?;
let json = serde_json::to_string_pretty(&inventory).unwrap();
std::fs::write("symbols.json", json).unwrap();
```

Use this when you need artifact evidence first.

Typical reasons:

- debugging whether a build exported the symbol you expected
- checking archive member provenance
- checking shared-library dependency edges

## Workflow 4: Validate Source-Derived Bindings Against Artifacts

```rust
use linc::{from_source_package, inspect_symbols, validate, SourcePackage};

let source = SourcePackage::default();
let binding = from_source_package(&source);
let inventory = inspect_symbols("build/libdemo.so")?;
let report = validate(&binding, &inventory);
```

This is the first serious consistency check between source intent and native reality.

For a split native surface:

```rust
use linc::{from_source_package, inspect_symbols, validate_many, SourcePackage};

let source = SourcePackage::default();
let binding = from_source_package(&source);
let core = inspect_symbols("build/libcore.so")?;
let support = inspect_symbols("build/libsupport.a")?;
let report = validate_many(&binding, &[core, support]);
```

## Workflow 5: Extract Just The Link Surface

```rust
let declared = &analysis.declared_link_surface;
let resolved = &analysis.resolved_link_plan;
```

This is the useful boundary if a downstream tool only wants:

- library names
- concrete artifact inputs
- framework inputs
- platform constraints
- ordering and link preference metadata

## Workflow 6: ABI-Sensitive Packages

For packages with important struct ABI:

```rust
use linc::{from_source_package, HeaderConfig, inspect_symbols, validate};

let result = HeaderConfig::new()
    .header("include/api.h")
    .probe_type_layout("struct api_context")
    .probe_type_layout("struct api_options")
    .process()?;

let source = linc::intake::adapters::from_binding_package(&result.package);
let binding = from_source_package(&source);
let inventory = inspect_symbols("build/libapi.so")?;
let report = validate(&binding, &inventory);
```

This gives you:

- layout evidence
- symbol-provider evidence
- a source-level contract that can be re-analyzed
- a separate validation decision surface

## Workflow 7: Downstream `fol` / `gerc` Consumption

The intended downstream pattern is:

1. `parc` produces `SourcePackage`
2. `linc` produces `LinkAnalysisPackage`
3. downstream generation reads source and link analysis in parallel
4. downstream generation reads `analysis.resolved_link_plan` to construct native link inputs
5. downstream generation may use validation output as a gate or diagnostic surface

That division keeps LINC focused on analysis and normalization rather than owning final build execution.

## Recommended Validation Gate

For serious native binding pipelines, a practical gate is:

- fail on `Missing`
- fail on `UnresolvedDeclaredLinkInputs`
- fail on `DuplicateProviders`
- inspect `DecorationMismatch`
- treat `WeakMatch` as policy-dependent

That is a pragmatic middle ground between "trust the source blindly" and "pretend current validation proves full ABI compatibility".
