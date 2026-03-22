# Header Processing

`HeaderConfig` exists as a repo-local bootstrap utility for turning raw header
sets into source-ish input that `linc` can analyze.

It is documented here because the code still exists and the test corpus still
uses it in places. It is not the architectural center of `linc`.

The intended architecture is:

- a frontend such as `parc` owns preprocessing, parsing, and declaration extraction
- `linc` consumes source-shaped input and produces evidence
- cross-package translation belongs outside `linc/src/**`

Read this chapter as operational guidance for a bootstrap utility, not as the
main contract that downstream tools should build around.

## What `HeaderConfig` Is Good For

`HeaderConfig` is useful when you need to:

- bootstrap the repository from real system or vendored headers
- drive difficult header fixtures without first teaching another frontend every edge case
- gather preprocessing output, extracted declarations, and probe evidence in one local pass

It is not the preferred downstream boundary.
The preferred downstream boundary is still:

```text
source-shaped input -> analyze_source_package(...) -> LinkAnalysisPackage
```

## Conceptual Domains

Even though `HeaderConfig` is one builder, it carries several distinct domains:

1. preprocessing environment
2. entry-header selection
3. declared native-link intent
4. ABI probe requests
5. origin-filtering policy

Those domains are useful when auditing scans because header failures often come
from one domain while the others are correct.

## Configuration Surface

The most important builder methods are:

| Method | Purpose |
|---|---|
| `header(path)` | Add an entry header |
| `include_dir(path)` | Add an include search path |
| `framework_dir(path)` | Add a framework search path |
| `library_dir(path)` | Add a native library search path |
| `define(name, value)` | Add a preprocessor define |
| `compiler(cmd)` | Override the driver used for preprocessing or probing |
| `flavor(f)` | Select dialect handling |
| `origin_filter(f)` | Keep only declarations from selected origins |
| `no_origin_filter()` | Keep declarations from every origin |
| `probe_type_layout(name)` | Request compiler-probed layout data |

Repeated path, define, link, constraint, and probe calls append in order.
The builder does not deduplicate for you.

## Validation Before Execution

`HeaderConfig::validate()` is a public preflight API.
`HeaderConfig::process()` validates before it attempts preprocessing,
extraction, or probing.

Treat invalid configuration as an operational error, not as one diagnostic
inside an otherwise usable result.

## What `process()` Does

Calling `.process()` performs a local bootstrap scan:

1. synthesize a temporary translation unit from the configured entry headers
2. preprocess it with the configured compiler and dialect settings
3. capture macros from the same environment
4. extract declarations and attached metadata
5. attach target, input, and declared link provenance
6. optionally probe requested layouts
7. optionally filter by origin

The returned `RawHeaderResult` contains:

- `package`
- `report.command`
- `report.args`
- `report.preprocessed_source`

## Practical Examples

### Entry Headers

```rust
let result = HeaderConfig::new()
    .header("include/api.h")
    .header("include/extra.h")
    .process()?;
```

The configured entry headers are treated as one scan unit, so order still
matters when headers depend on prior macros or type setup.

### Include Directories And Defines

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

### Native Link Intent

`HeaderConfig` can also attach declared native-link requirements while scanning:

- library search paths
- framework search paths
- library declarations
- object or artifact declarations
- preferred static/dynamic bias
- target constraints

That does not perform real linking. It preserves declared native dependency
intent so later analysis can reason about it.

## Policy

If you are writing new downstream code:

- do not treat `HeaderConfig` as the pipeline contract
- do not move cross-package translation into `linc/src/**`
- do not build new docs or examples around this path unless the point is specifically repository bootstrap

Use it when it helps the repository analyze difficult headers.
Do not mistake it for the long-term boundary between packages.
- the configured values are preserved in `package.inputs.defines`

## Compiler And Flavor

LINC uses the compiler as a preprocessor and ABI probe driver.

```rust
use linc::raw_headers::Flavor;

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
use linc::HeaderConfig;

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
