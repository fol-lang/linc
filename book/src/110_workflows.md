# End-To-End Workflows

This chapter ties the separate surfaces together into practical workflows.

## Workflow 1: Scan A Header And Save JSON

```rust
use bic::{HeaderConfig, to_json};

let result = HeaderConfig::new()
    .header("include/demo.h")
    .include_dir("include")
    .process()?;

let json = to_json(&result.package).unwrap();
std::fs::write("bindings.json", json).unwrap();
```

This is the baseline path for most automation.

The resulting file now contains:

- declarations
- macros
- layouts if requested
- link metadata
- diagnostics

## Workflow 2: Inspect A Native Artifact

```rust
use bic::inspect_symbols;

let inventory = inspect_symbols("build/libdemo.so")?;
let json = serde_json::to_string_pretty(&inventory).unwrap();
std::fs::write("symbols.json", json).unwrap();
```

Use this when you need artifact evidence first.

Typical reasons:

- debugging whether a build exported the symbol you expected
- checking archive member provenance
- checking shared-library dependency edges

## Workflow 3: Validate Bindings Against Artifacts

```rust
use bic::{inspect_symbols, validate};

let inventory = inspect_symbols("build/libdemo.so")?;
let report = validate(&package, &inventory);
```

This is the first serious consistency check between header intent and native reality.

For a split native surface:

```rust
use bic::{inspect_symbols, validate_many};

let core = inspect_symbols("build/libcore.so")?;
let support = inspect_symbols("build/libsupport.a")?;
let report = validate_many(&package, &[core, support]);
```

## Workflow 4: Extract Just The Link Surface

```rust
let link_surface = &package.link;
let json = serde_json::to_string_pretty(link_surface).unwrap();
std::fs::write("link-surface.json", json).unwrap();
```

This is a useful boundary if a downstream tool only wants:

- library names
- concrete artifact inputs
- framework inputs
- platform constraints
- ordering and link preference metadata

## Workflow 5: Preprocessed-Only Debugging

If a raw-header scan is confusing, break the problem in two:

1. produce or capture preprocessed source
2. run `scan-preprocessed`

```rust
use bic::PreprocessedInput;

let package = PreprocessedInput::from_file("debug.i")?
    .with_path("debug.h")
    .extract();
```

This isolates extraction behavior from compiler invocation behavior.

## Workflow 6: ABI-Sensitive Packages

For packages with important struct ABI:

```rust
use bic::{HeaderConfig, inspect_symbols, validate};

let result = HeaderConfig::new()
    .header("include/api.h")
    .probe_type_layout("struct api_context")
    .probe_type_layout("struct api_options")
    .process()?;

let inventory = inspect_symbols("build/libapi.so")?;
let report = validate(&result.package, &inventory);
```

This gives you:

- declaration extraction
- macro inventory
- layout evidence
- symbol-provider evidence

in one workflow.

## Workflow 7: Downstream `fol` Consumption

The intended downstream pattern is:

1. LINC library code produces `BindingPackage`
2. `fol` reads the package JSON
3. `fol` lowers `package.items` into generated bindings
4. `fol` reads `package.link` to construct native link inputs
5. `fol` may use validation output as a gate or diagnostic surface

That division keeps LINC focused on analysis and normalization rather than owning final build execution.

## Recommended Validation Gate

For serious native binding pipelines, a practical gate is:

- fail on `Missing`
- fail on `UnresolvedDeclaredLinkInputs`
- fail on `DuplicateProviders`
- inspect `DecorationMismatch`
- treat `WeakMatch` as policy-dependent

That is a pragmatic middle ground between "trust the headers blindly" and "pretend current validation proves full ABI compatibility".
