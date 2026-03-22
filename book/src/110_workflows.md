# End-To-End Workflows

This chapter ties the separate surfaces together into practical workflows.

Read the workflows in this order:

1. start from `SourcePackage` when a frontend already exists
2. attach artifact evidence only when you actually need it
3. keep repo-local bootstrap flows secondary

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

## Workflow 2: Test Or Harness Translation From PARC

The intended cross-package architecture is artifact-based, not shared-type based.

That means `linc` library code should not import `parc`.
Instead, a test, example, or external harness can translate a PARC-owned source
artifact into `linc` input and then run `analyze_source_package(...)`.

That translator should live outside `linc/src/**`.

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
use linc::{inspect_symbols, validate, SourcePackage};

let source = SourcePackage::default();
let binding = linc::intake::adapters::to_binding_package(&source);
let inventory = inspect_symbols("build/libdemo.so")?;
let report = validate(&binding, &inventory);
```

This is the first serious consistency check between source intent and native reality.

For a split native surface:

```rust
use linc::{inspect_symbols, validate_many, SourcePackage};

let source = SourcePackage::default();
let binding = linc::intake::adapters::to_binding_package(&source);
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

## Workflow 6: Downstream `fol` / `gerc` Consumption

The intended downstream pattern is:

1. `parc` produces a PARC-owned source artifact
2. tests/examples/harnesses translate that artifact into `linc` input
3. `linc` produces `LinkAnalysisPackage`
4. downstream generation reads source and link analysis in parallel
5. downstream generation reads `analysis.resolved_link_plan` to construct native link inputs
6. downstream generation may use validation output as a gate or diagnostic surface

That division keeps `linc` focused on analysis and evidence rather than parser
ownership or final build execution.

## Workflow 7: Repo-Local Raw-Header Bootstrap

```rust
use linc::analyze_source_package;
use linc::raw_headers::HeaderConfig;

let result = HeaderConfig::new()
    .header("include/demo.h")
    .include_dir("include")
    .process()?;

let source = linc::intake::adapters::from_binding_package(&result.package);
let analysis = analyze_source_package(&source);
```

This exists as a repo-local bootstrap path for difficult fixtures and
repository self-hosting. It is not the package boundary that downstream tools
should depend on.

## Workflow 7: ABI-Sensitive Packages

For packages with important struct ABI:

```rust
use linc::{inspect_symbols, validate};
use linc::raw_headers::HeaderConfig;

let result = HeaderConfig::new()
    .header("include/api.h")
    .probe_type_layout("struct api_context")
    .probe_type_layout("struct api_options")
    .process()?;

let source = linc::intake::adapters::from_binding_package(&result.package);
let binding = linc::intake::adapters::to_binding_package(&source);
let inventory = inspect_symbols("build/libapi.so")?;
let report = validate(&binding, &inventory);
```

This gives you:

- layout evidence
- symbol-provider evidence
- a source-level contract that can be re-analyzed
- a separate validation decision surface

## Recommended Validation Gate

For serious native binding pipelines, a practical gate is:

- fail on `Missing`
- fail on `UnresolvedDeclaredLinkInputs`
- fail on `DuplicateProviders`
- inspect `DecorationMismatch`
- treat `WeakMatch` as policy-dependent

That is a pragmatic middle ground between "trust the source blindly" and "pretend current validation proves full ABI compatibility".
