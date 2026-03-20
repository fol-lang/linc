# Macros And Layouts

Two of the most important "not just declarations" surfaces in LINC are:

- macro inventory
- macro provenance
- compiler-probed type layouts

Together they close a large part of the gap between syntax-only header extraction and ABI-aware binding generation.

## Macro Inventory

`BindingPackage.macros` captures macro definitions seen during raw-header scans.

Each `MacroBinding` carries:

- `name`
- `body`
- `function_like`
- `form`
- `kind`
- `category`
- optional parsed `value` for bindable integer/string constants

`BindingPackage.macro_provenance` carries aligned package-level provenance entries for captured
macros, including origin classification and source location where line-marker evidence is available.

### Macro Kind

Current kinds are:

- `Integer`
- `String`
- `Expression`
- `Other`

This is a structural classification of the macro body.

### Macro Category

Current categories are:

- `BindableConstant`
- `ConfigurationFlag`
- `AbiAffecting`
- `Unsupported`

This is a higher-level classification intended to help downstream generators and planners decide which macros are relevant.

## Why Macro Capture Matters

Many real C APIs encode essential information in macros:

- integer constants
- version identifiers
- feature toggles
- calling-convention selectors
- export/import annotations
- ABI-affecting packing or configuration knobs

Without macros, a binding package is often incomplete even if declaration extraction succeeded.

## Practical Macro Interpretation

Downstream tools should usually treat categories differently:

- `BindableConstant`: good candidates for generated constants
- `ConfigurationFlag`: environment and availability signals
- `AbiAffecting`: do not ignore; these may change layout or calling behavior
- `Unsupported`: evidence worth reporting, not blindly generating

## Layout Probing

`TypeLayout` currently stores:

- `name`
- `size`
- `align`

The layouts are produced by compiler-assisted probing.
That means they reflect the configured compiler environment rather than guessed sizes.

## Probe Through The API

You can probe layouts during a scan:

```rust
let result = HeaderConfig::new()
    .header("api.h")
    .probe_type_layout("struct api_context")
    .probe_type_layout("struct api_options")
    .process()?;
```

Or directly:

```rust
use linc::{probe_type_layouts, HeaderConfig};

let cfg = HeaderConfig::new()
    .header("api.h")
    .include_dir("include");

let report = probe_type_layouts(&cfg, &["struct api_context".into()])?;
println!("{:?}", report.layouts);
```

`AbiProbeReport` also preserves target/compiler identity metadata alongside the layouts.
That makes probe evidence auditable and safer to hand across process or repo boundaries.

For extensibility, the report also carries `subjects`.
Each `ProbeSubjectReport` keeps:

- the requested subject name
- its broad subject kind (`Type`, `Record`, or `Enum`)
- probe confidence
- record completeness when the subject is a record
- the measured `TypeLayout`

For record subjects, `fields` may also preserve named field offsets as compiler-measured evidence.

For bitfields, the current probe surface is intentionally partial:

- `bit_width` may be present
- `offset_bytes` may remain absent

That is deliberate. LINC preserves width evidence where it can, but does not guess a byte offset
for bitfields when the probe path cannot establish one safely.

The older `layouts` array is still preserved as a flattened compatibility surface.

## Probe Degradation Semantics

Probe requests do not all fail for the same reason.

When `HeaderConfig::process()` keeps a scan alive after probe trouble, downstream consumers should
distinguish:

- `ProbeUnavailable`: the requested subject did not have a safely probeable layout in the current
  compilation model
- `ProbeFailed`: the probe mechanism itself failed operationally or compiled invalid probe input

In practice, `ProbeUnavailable` is the expected result for shapes such as:

- incomplete record declarations
- intentionally opaque handles
- subjects where `sizeof` / `_Alignof` cannot be applied honestly

`ProbeFailed` is the stronger warning. It means the request path itself needs attention, for
example:

- an invalid probe subject string
- a compiler-side operational problem
- a probe translation unit that did not compile for reasons unrelated to an intentionally opaque
  type boundary

The package-level helper surface reflects this split directly:

- `BindingPackage::probe_unavailable_count()`
- `BindingPackage::probe_failure_count()`
- `BindingPackage::has_probe_unavailable_diagnostics()`

That allows a downstream generator to keep an intentional policy such as:

- tolerate `ProbeUnavailable` for explicitly opaque inputs
- require layouts for by-value ABI-sensitive records and typedef-backed value types
- treat any `ProbeFailed` result as suspicious until the probe path is fixed or explicitly waived

Current confidence/completeness semantics are intentionally conservative:

- `MeasuredLayout` means the compiler successfully measured the layout
- `Complete` means a record subject compiled as a complete type and therefore had a usable
  `sizeof` / `_Alignof` probe surface

Enum subjects also preserve:

- `enum_underlying_size`
- `enum_is_signed`

This gives downstream generators a concrete representation hint even before field-level enum
analysis exists in the declaration IR.

The repository also keeps dedicated probe contract fixtures for ABI-sensitive record and enum
subjects so record completeness and enum representation metadata are regression-tested as part of
the public transport surface.

## What Layouts Solve

Compiler-probed layouts are especially useful for:

- checking that opaque vs non-opaque modeling matches reality
- proving `sizeof` and `alignof` for important structs
- gating generation on ABI-sensitive records
- preserving ABI evidence in a transportable JSON package

## What Layouts Do Not Yet Solve

Current layout data is intentionally small.
It does not yet provide a full field-offset or bitfield-layout model.

So treat `TypeLayout` as:

- stronger than guessing
- not yet a complete ABI proof for all record shapes

That distinction is important when building a "full binder/linker" layer on top.
