# Header Processing

`HeaderConfig` is the main driver for turning raw headers into a `BindingPackage`.

It owns three separate concerns:

- how headers are preprocessed
- which declarations are treated as part of the bindable surface
- what native-link metadata should be attached to the resulting package

That description is directionally right, but still too coarse for long-term API stability.
For production use, it is better to think about `HeaderConfig` as five conceptual subdomains.

## Conceptual Subdomains

`HeaderConfig` currently groups the following configuration domains:

1. preprocessing inputs
2. binding-surface inputs
3. native link declarations
4. ABI probe requests
5. origin-filtering policy

The implementation is still a single builder type.
This categorization matters because future API cleanup work is likely to preserve these domains even if the concrete type layout changes.

The current library API also exposes borrowed views for these domains, so downstream code can
already treat them separately without waiting for a full type split:

- `HeaderConfig::preprocessing()`
- `HeaderConfig::binding_surface()`
- `HeaderConfig::linking()`
- `HeaderConfig::probing()`
- `HeaderConfig::filtering()`

### 1. Preprocessing Inputs

These options exist to make the header stack preprocess and parse correctly:

- `include_dir(...)`
- `define(...)`
- `compiler(...)`
- `flavor(...)`

These are about creating the right translation-unit environment.

### 2. Binding-Surface Inputs

These options define what surface is being scanned:

- `header(...)`

In practice, entry headers define the intended top-level bind surface, while origin filtering later controls how much transitive material remains in the final package.

### 3. Native Link Declarations

These options preserve native dependency intent alongside the extracted API:

- `framework_dir(...)`
- `library_dir(...)`
- `link_lib(...)`
- `link_static_lib(...)`
- `link_shared_lib(...)`
- `link_framework(...)`
- `link_object_file(...)`
- `link_static_artifact(...)`
- `link_shared_artifact(...)`
- `prefer_static_linking()`
- `prefer_dynamic_linking()`
- `target_constraint(...)`

These do not perform linking.
They describe and normalize the native surface the package expects.

### 4. ABI Probe Requests

These options request compiler-assisted ABI evidence:

- `probe_type_layout(...)`

These affect whether `package.layouts` is populated during the scan.

### 5. Origin-Filtering Policy

These options decide how much of the parsed declaration world survives into the returned package:

- `origin_filter(...)`
- `no_origin_filter()`

This is a post-extraction policy layer, not a preprocessing input.

## Defaults And Precedence Rules

For stable downstream use, `HeaderConfig` should be read as an append-oriented builder with a
small set of important defaults.

Current defaults:

- `origin_filter` starts as `Some(OriginFilter::default())`
- `preferred_link_mode` starts as `LinkResolutionMode::Default`
- `flavor` starts effectively as `GnuC11`
- `compiler` starts effectively as:
  - `clang` when the effective flavor is `ClangC11`
  - `gcc` otherwise
- all path, define, link, constraint, and probe lists start empty

Current precedence and accumulation rules:

- repeated calls to `header(...)`, `include_dir(...)`, `define(...)`, link-declaration methods,
  `target_constraint(...)`, and `probe_type_layout(...)` append in declaration order
- bulk builders such as `headers(...)` and `include_dirs(...)` use the same append semantics as
  repeated single-item builders
- no current builder method performs deduplication for you; if order or duplicates matter to
  your downstream flow, treat the builder input as authoritative
- `compiler(...)` overrides the compiler command that would otherwise be inferred from flavor
- `flavor(...)` overrides the default dialect assumption and also changes compiler inference when
  no explicit compiler has been provided
- `origin_filter(...)` installs an explicit filter policy
- `no_origin_filter()` disables filtering entirely, which is materially different from using the
  default filter

These rules matter because the configuration is preserved into multiple outputs:

- the actual preprocess/probe invocation
- `package.target`
- `package.inputs`
- `package.link`

In other words, `HeaderConfig` is not only execution input.
It is also provenance metadata.

## Validation Happens Before Execution

Configuration validation is part of the public library contract.

That means:

- `HeaderConfig::validate()` is a supported preflight API
- `HeaderConfig::process()` validates before it attempts preprocessing or extraction
- `probe_type_layouts(...)` validates before it attempts compiler probing

Downstream code should treat invalid configuration as an operational error, not as a diagnostic
inside an otherwise successful result.

## What `process()` Does

Calling `.process()` performs this sequence:

1. Build a temporary translation unit that includes the configured entry headers
2. Run the configured compiler/preprocessor through `pac`
3. Capture macro definitions from the same header set
4. Extract binding items from the parsed translation unit
5. Attach target/input/link metadata
6. Optionally probe requested type layouts
7. Optionally filter items by source origin

The returned `RawHeaderResult` contains:

- `package`: the extracted result
- `report.command`: compiler executable used
- `report.args`: effective preprocessor arguments
- `report.preprocessed_source`: the exact source seen by the parser

## Core Configuration Surface

The most important builder methods are:

| Method | Purpose |
|---|---|
| `header(path)` | Add an entry header |
| `include_dir(path)` | Add an include search path |
| `framework_dir(path)` | Add a framework search path |
| `library_dir(path)` | Add a native library search path |
| `define(name, value)` | Add a preprocessor define |
| `compiler(cmd)` | Override the compiler/preprocessor driver |
| `flavor(f)` | Select C dialect handling |
| `origin_filter(f)` | Use a custom origin filter |
| `no_origin_filter()` | Keep declarations from every origin |
| `probe_type_layout(name)` | Request compiler-probed layout data |

For downstream API design, it is often cleaner to inspect one of the borrowed domain views than
to pass the full `HeaderConfig` everywhere.

## Builder Naming Policy

`HeaderConfig` still carries some short historical builder names, but the intended naming direction
is now explicit.

Preferred style for new downstream code:

- `entry_header(...)`
- `add_include_dir(...)`
- `add_framework_dir(...)`
- `add_library_dir(...)`
- `define_flag(...)`
- `define_value(...)`
- `link_library(...)`
- `request_probe_type_layout(...)`

Still supported for compatibility:

- `header(...)`
- `include_dir(...)`
- `framework_dir(...)`
- `library_dir(...)`
- `define(...)`
- `link_lib(...)`
- `probe_type_layout(...)`

The rule is simple: old short names remain valid, but clearer names are preferred for new code and
new examples.

## Entry Headers

Entry headers define the top-level API surface you are scanning.

```rust
let result = HeaderConfig::new()
    .header("include/api.h")
    .header("include/extra.h")
    .process()?;
```

Internally, LINC synthesizes a temporary source file containing `#include` lines for each entry header.

That means:

- order matters when headers depend on previous macro or type setup
- multiple entry headers are treated as one scan unit
- diagnostics and origin filtering are still tracked back to source origins

## Include Directories And Defines

Headers almost always depend on compile-time environment.
If your scan omits that environment, the extracted package is unreliable.

```rust
let result = HeaderConfig::new()
    .header("api.h")
    .include_dir("vendor/include")
    .include_dir("generated/include")
    .define("API_VERSION", Some("3".into()))
    .define("USE_EXPERIMENTAL", None)
    .process()?;
```

Notes:

- `define("NAME", None)` corresponds to `-DNAME`
- `define("NAME", Some("VALUE".into()))` corresponds to `-DNAME=VALUE`
- the configured values are preserved in `package.inputs.defines`

## Compiler And Flavor

LINC uses the compiler as a preprocessor and ABI probe driver.

```rust
use bic::raw_headers::Flavor;

let result = HeaderConfig::new()
    .header("api.h")
    .compiler("clang")
    .flavor(Flavor::ClangC11)
    .process()?;
```

Flavor affects parsing expectations and extension handling:

- `GnuC11`
- `ClangC11`
- `StdC11`

In general:

- use `ClangC11` when the header stack is written for Clang tooling
- use `GnuC11` when the project assumes GCC-style C extensions
- use `StdC11` only when you want a stricter source profile

If `compiler(...)` is not set explicitly, LINC currently infers the driver from the effective
flavor:

- `ClangC11` -> `clang`
- `GnuC11` / `StdC11` -> `gcc`

## Native Link Inputs During Scan

The scan phase can also record the native inputs that the extracted API expects.

Examples:

```rust
let result = HeaderConfig::new()
    .header("sqlite3.h")
    .library_dir("/opt/sqlite/lib")
    .link_lib("sqlite3")
    .prefer_dynamic_linking()
    .process()?;
```

Or with concrete artifacts:

```rust
let result = HeaderConfig::new()
    .header("engine.h")
    .link_object_file("build/engine_shim.o")
    .link_static_artifact("build/libengine_support.a")
    .link_shared_artifact("build/libengine.so")
    .process()?;
```

These declarations are preserved in `package.link`.
The scan does not link anything by itself.
It records the intent and the normalized link surface.

Link declarations are append-only and declaration-ordered.
If you mix library names, frameworks, and concrete artifacts, that original declared order is
preserved in `package.link.ordered_inputs`.

## Frameworks And Platform Constraints

For Apple-style native surfaces:

```rust
let result = HeaderConfig::new()
    .header("mykit.h")
    .framework_dir("/Library/Frameworks")
    .link_framework("CoreFoundation")
    .target_constraint("x86_64-apple-darwin")
    .process()?;
```

Platform constraints are simple strings today.
They are preserved so downstream consumers can decide whether a package applies to the current target.

## Layout Probing During Scan

You can request ABI layout facts directly in the scan:

```rust
let result = HeaderConfig::new()
    .header("api.h")
    .probe_type_layout("struct api_context")
    .probe_type_layout("struct api_config")
    .process()?;
```

The resulting package will include `package.layouts`.

This is the preferred path when the binding package needs to carry extracted declarations and layout evidence together.

Repeated probe requests append in order.
No implicit deduplication is performed today.

## Diagnostics And Partial Success

LINC is intentionally diagnostic-heavy.
A scan can succeed structurally while still recording unsupported constructs.

Always inspect:

- `package.diagnostics`
- `report.preprocessed_source`

Treat a "successful" scan with important diagnostics as an incomplete binding package, not a final truth.

## Raw Header Result Example

```rust
use bic::HeaderConfig;

let result = HeaderConfig::new()
    .header("api.h")
    .include_dir("include")
    .process()?;

println!("compiler: {}", result.report.command);
println!("argv: {:?}", result.report.args);
println!("items: {}", result.package.items.len());
println!("diagnostics: {}", result.package.diagnostics.len());
```

## Failure Modes To Expect

The most common failure categories are:

- header not found
- compiler/preprocessor invocation mismatch
- missing defines or include paths
- unsupported source constructs reduced to diagnostics
- layout probe requests for names the compiler cannot resolve

When debugging, reduce the problem in this order:

1. confirm the compiler command and args
2. inspect the preprocessed source
3. disable origin filtering if declarations appear missing
4. compare the extracted item set against the original header intent
