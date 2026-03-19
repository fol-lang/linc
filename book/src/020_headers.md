# Header Processing

`HeaderConfig` is the main driver for turning raw headers into a `BindingPackage`.

It owns three separate concerns:

- how headers are preprocessed
- which declarations are treated as part of the bindable surface
- what native-link metadata should be attached to the resulting package

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

## Entry Headers

Entry headers define the top-level API surface you are scanning.

```rust
let result = HeaderConfig::new()
    .header("include/api.h")
    .header("include/extra.h")
    .process()?;
```

Internally, `bic` synthesizes a temporary source file containing `#include` lines for each entry header.

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

`bic` uses the compiler as a preprocessor and ABI probe driver.

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

## Diagnostics And Partial Success

`bic` is intentionally diagnostic-heavy.
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
