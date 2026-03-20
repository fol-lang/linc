# BIC vs bindgen — An Architectural Deep Dive

This document explains how BIC and bindgen approach the same fundamental problem — making C
libraries usable from Rust — through radically different architectures. It walks through
parsing, type extraction, ABI discovery, symbol inspection, validation, linking, and code
generation step by step, showing exactly where each tool does its work and what it produces.

---

## The Core Difference in One Sentence

**bindgen is a transpiler**: it reads a C header and writes Rust code.
**BIC is an analysis engine**: it reads a C header *and* the compiled artifact, cross-references
them, and produces a structured evidence report that downstream tools can act on.

bindgen answers: "what does this header say?"
BIC answers: "what does this header say, does the compiled library agree, and how confident
should you be?"

---

## 1. Parsing

### How bindgen parses C

bindgen does not have its own parser. It shells out to **libclang** — the C/C++ compiler
frontend from the LLVM project — and walks the resulting AST through the `clang-sys` crate.
This means:

- Every machine that builds with bindgen needs a working libclang installation (headers + shared
  library or static archive). On some distros this is `libclang-dev`, on macOS it comes with
  Xcode, on Windows you install LLVM.
- bindgen inherits libclang's full C11/C17 and partial C++17 parsing capability. It correctly
  handles platform-specific extensions, GNU attributes, MSVC declspec, Objective-C syntax.
- The downside: libclang is a massive dependency. It drags in the entire Clang frontend at
  either build time (static linking) or runtime (dynamic loading via `dlopen`). This makes
  cross-compilation painful and CI setup non-trivial.
- For `#define` macros, libclang provides almost no useful information (it operates on the
  post-preprocessed token stream). bindgen works around this with the `cexpr` crate — a
  standalone C expression evaluator — to parse integer and string constant macros. Complex
  macros (casts, nested expressions, function-like macros) either fail silently or require
  an opt-in `clang_macro_fallback` that compiles each macro in isolation.

### How BIC parses C

BIC uses **PAC** (`follang/pac`), a custom C parser written entirely in Rust. The pipeline is:

1. BIC invokes the system's C compiler (gcc or clang) **only as a preprocessor** (`-E` flag)
   to expand includes, evaluate `#ifdef` blocks, and produce a single preprocessed translation
   unit.
2. PAC parses that preprocessed source into a full C AST (`TranslationUnit`).
3. BIC's `Extractor` walks the AST and converts C declarations into its own IR.

The implications are significant:

- **No libclang dependency**. Any machine with gcc or clang (which every C development
  environment already has) can run BIC. No special dev packages, no LLVM install, no
  `LIBCLANG_PATH` environment variable. The parser is just a Rust crate.
- **Preprocessing is separated from parsing**. BIC explicitly runs the preprocessor as a
  subprocess, captures the preprocessed source (preserving it in `PreprocessingReport`), and
  then parses it. This means you can inspect exactly what the preprocessor produced, you can
  feed already-preprocessed `.i` files directly via `PreprocessedInput`, and you can reproduce
  issues without the original system headers.
- **Macro capture is first-class**. During preprocessing, BIC captures `#define` macros with
  their bodies, classifies them (`BindableConstant`, `ConfigurationFlag`, `AbiAffecting`,
  `Unsupported`), and when safe, parses their values into typed representations (`MacroValue::Integer(42)`,
  `MacroValue::String("hello")`). Function-like macros are identified and marked.
- The tradeoff: PAC only handles C (not C++, not Objective-C). BIC is deliberately C-only.

### What this means practically

If you're binding a C++ library, you must use bindgen (or cbindgen, or cxx). If you're binding
a C library — which is the vast majority of system libraries, embedded SDKs, kernel interfaces,
and database engines — BIC gives you a lighter-weight, more inspectable pipeline with no system
dependency beyond a C compiler you already have.

---

## 2. Internal Representation

### bindgen's IR

After parsing with libclang, bindgen builds an internal **item graph**. Each node is an `Item`
identified by a unique `ItemId`. Items have an `ItemKind`:

- `Module` — C++ namespaces mapped to Rust modules
- `Type` — with many `TypeKind` sub-variants (integers, floats, pointers, function pointers,
  aliases, arrays, compound types, template instantiations)
- `Function` — ABI info, mangled name, link to its function-pointer type
- `Var` — constants and static variables

Items reference each other by `ItemId`, forming a graph. A `Trace` trait enumerates outgoing
edges (base members, field types, parameter types) for graph traversal. This graph is the
input to bindgen's analysis passes and code generation.

The IR is **internal and ephemeral**. It exists only in memory during a single bindgen
invocation. There is no serialization format, no schema version, no way to inspect it after
the run completes (beyond debug flags like `--emit-ir`).

### BIC's IR

BIC's IR is defined in `ir.rs` and centers on `BindingPackage` — a top-level container that
is fully serializable to JSON via serde:

```rust
pub struct BindingPackage {
    pub schema_version: u32,
    pub bic_version: String,
    pub source_path: Option<String>,
    pub target: BindingTarget,         // compiler identity
    pub inputs: BindingInputs,         // headers, include dirs, defines
    pub items: Vec<BindingItem>,       // extracted declarations
    pub diagnostics: Vec<Diagnostic>,  // warnings/errors from extraction
    pub macros: Vec<MacroBinding>,     // captured preprocessor macros
    pub layouts: Vec<TypeLayout>,      // compiler-probed type layouts
    pub link: BindingLinkSurface,      // native link requirements
    pub provenance: Vec<DeclarationProvenance>,  // per-item source origin
}
```

`BindingItem` is an enum:

```rust
pub enum BindingItem {
    Function(FunctionBinding),    // name, params, return type, calling convention, variadic
    Record(RecordBinding),        // struct/union with fields, layout evidence
    Enum(EnumBinding),            // variants with values, representation evidence
    TypeAlias(TypeAliasBinding),  // typedef with canonical resolution chain
    Variable(VariableBinding),    // extern globals
    Unsupported(UnsupportedItem), // recognized but unmodelable constructs
}
```

The type system (`BindingType`) maps C types faithfully:

```rust
pub enum BindingType {
    Void, Bool, Char, SChar, UChar, Short, UShort, Int, UInt,
    Long, ULong, LongLong, ULongLong, Float, Double, LongDouble,
    Pointer { pointee, const_pointee, qualifiers },
    Array(inner, Option<size>),
    FunctionPointer { return_type, parameters, variadic },
    Qualified { ty, qualifiers },
    TypedefRef(name), RecordRef(name), EnumRef(name), Opaque(name),
}
```

The critical difference: **BIC's IR is a durable, versioned data contract**. You serialize it
with `to_json()`, store it, transmit it, diff it, feed it to other tools. A `BindingPackage`
from BIC version 0.1.0 will deserialize in future versions as long as `schema_version` is
compatible. bindgen's IR is a transient in-memory structure that only lives long enough to
generate Rust code.

---

## 3. ABI Discovery

This is where BIC and bindgen diverge most dramatically.

### How bindgen discovers ABI

bindgen relies entirely on **libclang's type layout information**. When libclang parses a
struct, it knows the size, alignment, and field offsets because it is literally the C compiler
frontend — it has already computed the layout using the target's ABI rules.

bindgen reads these values from libclang's API and uses them to:

1. Generate `#[repr(C)]` structs with correct field order and padding.
2. Emit **layout test functions** — `#[test]` functions that assert `size_of::<Foo>()`,
   `align_of::<Foo>()`, and `offset_of!(Foo, field)` match the C values. These tests run at
   `cargo test` time on the target machine.

The layout tests look like this in generated code:

```rust
#[test]
fn bindgen_test_layout_point() {
    assert_eq!(size_of::<point>(), 8usize);
    assert_eq!(align_of::<point>(), 4usize);
    assert_eq!(offset_of!(point, x), 0usize);
    assert_eq!(offset_of!(point, y), 4usize);
}
```

This is a good approach but it has a timing problem: **the verification happens after code
generation**, when someone runs `cargo test`. If no one runs the tests (common in CI for
`-sys` crates that just vendor pre-generated bindings), layout bugs go undetected. And the
tests verify the layout on the machine running `cargo test`, which may differ from the target.

### How BIC discovers ABI

BIC takes a fundamentally different approach. The `probe.rs` module implements
**compiler-assisted probing**:

1. BIC generates a temporary C source file that uses `sizeof()`, `_Alignof()`, and `offsetof()`
   to print layout facts for requested types.
2. It compiles this file using the same compiler (gcc/clang) and flags that would build the
   real library.
3. It executes the resulting binary and parses its output.
4. The results are stored as `TypeLayout` and `ProbeSubjectReport` in the `BindingPackage`.

Here's what the generated probe source looks like (simplified):

```c
#include <stdio.h>
#include <stddef.h>
#include "mylib.h"

int main(void) {
    printf("L\t%s\t%zu\t%zu\t-\t-\n", "struct point", sizeof(struct point), _Alignof(struct point));
    printf("F\t%s\t%s\t%zu\t-\n", "struct point", "x", offsetof(struct point, x));
    printf("F\t%s\t%s\t%zu\t-\n", "struct point", "y", offsetof(struct point, y));
    return 0;
}
```

For enums, the probe also discovers the underlying size and signedness:

```c
printf("L\t%s\t%zu\t%zu\t%zu\t%d\n", "enum color",
    sizeof(enum color), _Alignof(enum color),
    sizeof(enum color),
    ((enum color)-1) < ((enum color)0) ? 1 : 0);
```

The probe results are structured data:

```rust
pub struct ProbeSubjectReport {
    pub name: String,
    pub kind: ProbeSubjectKind,       // Type, Record, or Enum
    pub confidence: ProbeConfidence,   // MeasuredLayout
    pub record_completeness: Option<RecordCompleteness>,
    pub enum_underlying_size: Option<u64>,
    pub enum_is_signed: Option<bool>,
    pub fields: Vec<ProbedFieldLayout>,
    pub layout: TypeLayout,           // { name, size, align }
}
```

This matters because the layout facts come from **the actual compiler with the actual flags**,
not from libclang which may have different defaults. And the facts are captured **at analysis
time**, not deferred to test time.

---

## 4. Symbol Inspection

### bindgen: no symbol inspection

bindgen does not look at compiled artifacts at all. It reads headers and produces Rust code.
Whether the functions declared in the header actually exist in any `.so`, `.a`, `.dylib`, or
`.o` file is entirely outside its scope. You find out at link time when `cargo build` fails
with "undefined reference to `foo`".

### BIC: first-class symbol inventory

BIC's `symbols.rs` module (behind the `symbols` feature flag) reads native artifacts using the
`object` crate and produces a `SymbolInventory`:

```rust
pub struct SymbolInventory {
    pub artifact_path: String,
    pub format: ArtifactFormat,       // ElfObject, ElfSharedLibrary, MachODylib, CoffObject, ...
    pub platform: ArtifactPlatform,   // Elf, MachO, Windows
    pub kind: ArtifactKind,           // Object, StaticLibrary, SharedLibrary, Executable
    pub capabilities: ArtifactCapabilities,  // { exports_symbols, imports_symbols }
    pub dependency_edges: Vec<String>,       // DT_NEEDED entries for ELF
    pub symbols: Vec<SymbolEntry>,
}
```

Each `SymbolEntry` captures:

```rust
pub struct SymbolEntry {
    pub name: String,              // normalized match key
    pub raw_name: Option<String>,  // original name (e.g., with decorations)
    pub version: Option<String>,   // ELF symbol versioning (e.g., GLIBC_2.17)
    pub direction: SymbolDirection, // Exported or Imported
    pub reexported_via: Vec<String>,
    pub alias_of: Option<String>,
    pub function_abi: Option<FunctionAbiHint>,  // parameter_count, return_size, parameter_sizes
    pub visibility: SymbolVisibility,  // Default, Hidden, Protected, Internal
    pub is_function: bool,
    pub binding: SymbolBinding,    // Local, Global, Weak
    pub size: Option<u64>,         // from ELF symbol table
    pub section: Option<String>,
    pub archive_member: Option<String>,  // which .o inside a .a
}
```

The artifact inspection handles:

- **ELF**: object files (`.o`), static libraries (`.a`, walking archive members), shared
  libraries (`.so`, including `DT_NEEDED` dependency detection via `readelf -d`)
- **Mach-O**: object files, dylibs, static archives
- **COFF/PE**: object files, DLLs, import libraries (detecting `__imp_` prefixed symbols)

For static archives (`.a`), BIC iterates through each archive member, parses each `.o` inside,
deduplicates symbols, and records which member each symbol came from.

This gives downstream tools a complete picture of what a native artifact actually provides —
not what a header claims it provides, but what the compiled binary actually exports.

---

## 5. Validation

### bindgen: no validation

bindgen generates code and hopes for the best. If the header declares `void foo(int x)` but
the library was compiled with `void foo(int x, int y)`, you get undefined behavior at runtime.
bindgen has no mechanism to detect this.

### BIC: three-phase validation engine

BIC's `validate.rs` compares a `BindingPackage` against one or more `SymbolInventory` values
through three phases:

**Phase 1 — Provider Discovery**: For each declared function and variable, BIC searches the
symbol inventories for a matching exported symbol. It considers:

- Direct name matches
- Decorated names (e.g., `_foo` prefixed symbols, `__imp_foo` for Windows)
- Weak vs global binding
- Visibility (default, hidden, protected)
- Re-exported symbols

**Phase 2 — Symbol Identity**: For each candidate match, BIC classifies the evidence:

- `ExactExported` — global symbol with default visibility, highest confidence
- `AbiShapeVerified` — exact match with ABI shape confirmation
- `WeakExported` — weak binding (may be overridden)
- `HiddenProvider` — exists but not externally visible
- `DecoratedCandidate` — matched via name decoration normalization
- `ReexportedCandidate` — available through a dependency chain
- `DuplicateVisibleProviders` — multiple libraries export the same symbol
- `MissingProvider` — not found in any inventory

**Phase 3 — ABI Evidence**: When symbol metadata is available, BIC checks ABI-level details:

For **variables**: compares the declared type's expected size against the symbol's size from
the object file's symbol table.

For **functions**: BIC constructs a `FunctionAbiHint` from the declaration (parameter count,
return type size, per-parameter sizes) and compares it against any ABI hints from the symbol:

```rust
pub struct RoutineAbiEvidence {
    pub evidence_kind: Option<RoutineAbiEvidenceKind>,
    pub confidence: Option<RoutineAbiConfidence>,
    pub expected_parameter_count: Option<usize>,
    pub observed_parameter_count: Option<usize>,
    pub expected_return_size: Option<u64>,
    pub observed_return_size: Option<u64>,
    pub expected_parameter_sizes: Vec<Option<u64>>,
    pub observed_parameter_sizes: Vec<Option<u64>>,
}
```

The evidence kinds form a completeness ladder:

- `ParameterCountOnly` — only count could be compared
- `ReturnShapeOnly` — only return size could be compared
- `ParameterCountAndReturnShape` — both count and return size verified
- `ParameterCountAndParameterShapes` — count and per-parameter sizes verified
- `FullyShaped` — everything verified (count + return + all parameter sizes)
- `Mismatch` — a verifiable disagreement was found

The result is a `ValidationReport` with per-declaration match status and confidence:

```rust
pub struct ValidationReport {
    pub phases: Vec<ValidationPhaseReport>,
    pub entries: Vec<ValidationEntry>,
    pub summary: ValidationSummary,
    pub matches: Vec<SymbolMatch>,
}
```

This is not a boolean pass/fail. It's structured evidence that a downstream build system can
use to make policy decisions: "proceed only if all critical symbols have High confidence",
"warn on WeakMatch", "fail on AbiShapeMismatch".

---

## 6. Linking

### bindgen: not its problem

bindgen generates `extern "C"` blocks in Rust. Linking is the responsibility of the build
system — typically a `build.rs` that uses `pkg-config`, `cmake`, or manual
`cargo:rustc-link-lib` directives. bindgen has no opinion about where `libfoo.so` lives or
whether it's compatible.

### BIC: structured link surface

BIC models linking as a first-class part of the binding contract. `HeaderConfig` accepts
library declarations:

```rust
config
    .library("z", LinkLibraryKind::Dynamic)
    .framework("Security")                    // Apple framework
    .artifact("/usr/lib/libcustom.a")         // concrete path
    .library_dir("/opt/custom/lib")           // search path
    .platform_constraint("x86_64-unknown-linux-gnu")
```

These declarations flow into `BindingLinkSurface` inside the `BindingPackage`:

```rust
pub struct BindingLinkSurface {
    pub libraries: Vec<LinkLibrary>,      // name + static/dynamic preference
    pub frameworks: Vec<LinkFramework>,   // Apple-style
    pub artifacts: Vec<LinkArtifact>,     // concrete paths
    pub library_dirs: Vec<String>,        // search roots
    pub framework_dirs: Vec<String>,
    pub ordered_inputs: Vec<LinkInput>,   // declaration order preserved
    pub preferred_mode: LinkResolutionMode,
    pub native_surface_kind: NativeSurfaceKind,
    pub platform_constraints: Vec<String>,
}
```

The `link_plan.rs` module then resolves these declared requirements against available
inventories:

```rust
pub struct ResolvedLinkPlan {
    pub preferred_mode: LinkResolutionMode,
    pub platform_constraints: Vec<String>,
    pub inputs: Vec<LinkInput>,
    pub requirements: Vec<ResolvedLinkRequirement>,
    pub transitive_dependencies: Vec<String>,
}
```

Each requirement tracks its resolution status:

```rust
pub enum RequirementResolution {
    Unresolved,  // no provider found
    Resolved,    // exactly one provider matched
    Ambiguous,   // multiple providers found
}
```

When given symbol inventories (from inspecting the actual `.so`/`.a` files), BIC matches
providers by artifact path, library name pattern, or framework name. It also extracts
transitive dependencies (via ELF `DT_NEEDED` entries) so downstream tools know the full
dependency chain.

This means a downstream build tool can read the `ResolvedLinkPlan` and generate the correct
`cargo:rustc-link-lib`, `cargo:rustc-link-search`, and framework flags automatically, with
confidence that the providers actually exist and export the expected symbols.

---

## 7. Code Generation

### How bindgen generates Rust

bindgen's code generation is its primary purpose. The `codegen` module walks the IR graph and
uses the `quote` crate (quasi-quoting) to construct Rust token streams. Post-processing passes
run via `syn` (the Rust syntax tree library) to clean up the output. The result goes through
either `rustfmt` or `prettyplease` for formatting.

bindgen generates:

- `#[repr(C)]` structs with explicit padding fields where needed
- Native `union` types (if all fields are `Copy`) or `ManuallyDrop`-based wrappers
- `extern "C" { }` blocks with function declarations
- Enum representations in six different styles (constified constants, newtypes, Rust enums, bitfield
  newtypes) configurable per-enum via regex patterns
- Typedef aliases as `type`, `#[repr(transparent)]` newtypes, or newtypes with `Deref` impl
- Bitfield accessor methods (getter + setter for each bitfield)
- Layout test functions
- Optionally: dynamic loading structs (a struct with `dlsym`-loaded function pointers)
- Optionally: Rust modules mirroring C++ namespaces
- Optionally: wrapper functions for `static inline` C functions (by generating a C source file
  with non-inline wrappers that the build must compile separately)

The level of customization is enormous — the `Builder` has 100+ configuration methods, and
the `ParseCallbacks` trait provides 20 hooks for intercepting and modifying the generation
process at every stage.

### How BIC generates Rust

BIC's code generation is **optional** (behind the `codegen` feature flag) and intentionally
minimal. The `codegen_rust.rs` module is a straightforward `RustEmitter` that walks
`BindingPackage.items` and emits Rust source as a plain `String`:

```rust
pub fn emit_rust_ffi(package: &BindingPackage) -> String {
    RustEmitter::new().emit(package)
}
```

It generates:

- `pub type` aliases for typedefs
- `pub type EnumName = ::core::ffi::c_int;` with `pub const` for each variant
- `#[repr(C)] pub struct/union` with fields
- `extern "C" { }` blocks for functions (grouped by calling convention)
- `pub static` declarations for external variables

The code generation uses `::core::ffi::*` types (not `std::os::raw::*`), handles deep pointer
chains, flexible array members (as pointers with comments), opaque structs (`[u8; 0]`), and
function pointers with variadic parameters.

BIC's codegen is deliberately unsophisticated. It doesn't generate layout tests, derive macros,
padding fields, or bitfield accessors. The philosophy is that code generation is a downstream
concern — BIC's job is to produce the `BindingPackage` contract with all the evidence, and
the consumer (like `fol`) decides how to generate code from it. The built-in `emit_rust_ffi`
is a convenience for simple cases, not the intended production path.

---

## 8. The Full Pipeline, Side by Side

### bindgen's pipeline

```
mylib.h
  │
  ▼
libclang (parsing + layout computation)
  │
  ▼
IR graph (Items connected by ItemId)
  │
  ▼
Analysis passes (derive eligibility, bitfield allocation)
  │
  ▼
Code generation (quote + syn)
  │
  ▼
Formatting (rustfmt or prettyplease)
  │
  ▼
bindings.rs  ← this is the final product
```

Everything happens in a single invocation. Input: header file. Output: Rust source file. Done.

### BIC's pipeline

```
mylib.h  +  compiler flags
  │
  ▼
C preprocessor (gcc -E / clang -E)
  │
  ▼
Preprocessed source + captured macros
  │
  ▼
PAC parser (C AST)
  │
  ▼
Extractor (AST → BindingItem)
  │
  ▼
BindingPackage (items + diagnostics + macros + link surface)
  │
  ├──► [optional] ABI probe (compile + run sizeof/alignof)
  │      └──► TypeLayout + field offsets → package.layouts
  │
  ├──► [optional] Symbol inspection (read .so/.a/.o/.dylib)
  │      └──► SymbolInventory
  │
  ├──► [optional] Validation (package × inventory)
  │      └──► ValidationReport (per-item match status + confidence)
  │
  ├──► [optional] Link planning (package × inventories)
  │      └──► ResolvedLinkPlan (provider resolution)
  │
  ├──► to_json() → deterministic JSON contract
  │
  └──► [optional] emit_rust_ffi() → Rust source
```

Each stage is independently callable. You can extract without probing. You can validate without
generating code. You can serialize to JSON without any of the optional steps. Each intermediate
result is a structured, serializable value.

---

## 9. Error and Diagnostic Model

### bindgen

bindgen uses Rust's standard `Result` and `panic!` for hard errors. Warnings are printed to
stderr during generation. There's no structured diagnostic output — you read the terminal. If
bindgen can't handle a construct (say, a C++ template), it silently skips it or emits an opaque
placeholder type.

### BIC

BIC separates **operational failures** from **analysis findings**:

- Hard failures (can't find the preprocessor, can't parse the file, can't read an artifact)
  are returned as `Err(BicError)` with typed variants:

  ```rust
  pub enum BicError {
      NoHeaders,
      NoProbeTypes,
      ProbeCompile { compiler, stderr },
      ProbeExecution { reason },
      ProbeOutput { reason },
      SymbolRead { path, reason },
      UnsupportedFormat { path, format },
      SchemaVersion { found, supported },
      Io(std::io::Error),
      Json(serde_json::Error),
  }
  ```

- Analysis findings are carried **inside the returned data**, not as exceptions:

  ```rust
  pub struct Diagnostic {
      pub kind: DiagnosticKind,    // PreprocessingFailed, ParseFailed, DeclarationUnsupported, ...
      pub severity: Severity,      // Warning or Error
      pub message: String,
      pub item_name: Option<String>,
      pub artifact: Option<String>,
      pub location: Option<(Option<String>, usize)>,
  }
  ```

  Diagnostics live in `package.diagnostics`. Validation mismatches live in
  `report.entries`. These are data, not control flow. A downstream tool can filter, count,
  or policy-gate on them programmatically.

---

## 10. What Each Tool Cannot Do

### bindgen cannot:

- Verify that declared functions exist in the compiled library
- Tell you if the header's struct layout matches the library's struct layout on your target
  (until you run `cargo test`, by which time you've already generated and committed the code)
- Model the link surface (which `.so` provides which symbol, transitive dependencies)
- Produce machine-readable analysis output for other tools to consume
- Work without libclang installed

### BIC cannot:

- Handle C++ in any form
- Handle Objective-C
- Produce the level of Rust codegen customization bindgen offers (enum styles, derive control,
  namespace mapping, dynamic loading structs, bitfield accessors)
- Run on Windows as a primary target (COFF/PE parsing exists but is deprioritized)
- Generate bindings for `static inline` functions

---

## 11. When to Use Which

**Use bindgen when:**

- You're binding a C++ library (bindgen is your only real option in the Rust ecosystem)
- You need production-quality Rust codegen with fine-grained control over enum styles, derive
  traits, naming conventions, and module structure
- You're writing a `-sys` crate and want a single-command header-to-Rust solution
- You need Objective-C support
- Your CI already has libclang and you don't care about the dependency

**Use BIC when:**

- You're binding a C library and want to **verify** that the header matches the compiled artifact
  before trusting the bindings
- You're building a binding pipeline where multiple tools consume the extraction output (BIC
  produces JSON contracts, not just Rust code)
- You need structured link planning — knowing which library provides which symbol, whether
  providers are resolved, and what the transitive dependency chain looks like
- You want to avoid the libclang system dependency
- You want compiler-probed ABI evidence attached to the binding metadata, not deferred to
  test time
- You're working in a build system (like fol) that generates its own code from structured data
  rather than consuming raw Rust source

**Use both when:**

- BIC can validate the binding surface and produce confidence evidence, then bindgen (or a
  custom generator consuming BIC's JSON) can generate the actual Rust code. BIC occupies the
  verification layer that bindgen doesn't have.

---

## 12. Dependency and Build Cost

### bindgen

```toml
# Cargo.toml (build-dependency)
[build-dependencies]
bindgen = "0.72"
```

Transitive deps: `clang-sys`, `cexpr`, `quote`, `syn`, `proc-macro2`, `regex`, `itertools`,
`bitflags`, `shlex`, `rustc-hash`, optionally `prettyplease` and `log`.

System requirement: **libclang** (typically 50-200 MB installed). Build times are dominated by
`syn` and `quote` compilation.

### BIC

```toml
# Cargo.toml
[dependencies]
bic = { version = "0.1", features = ["symbols", "codegen"] }
```

Transitive deps: `pac` (git), `serde`, `serde_json`, optionally `object` (for symbol
inspection).

System requirement: a C compiler (gcc or clang) — which you already have if you're doing
C interop. No libclang, no LLVM, no special packages.

BIC is substantially lighter to build and deploy.
