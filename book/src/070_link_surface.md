# Link Surface

`BindingPackage.link` is the normalized native-link surface attached to a scan.

This is one of the most important additions in the current `bic` architecture because binding generation alone is not enough.
Downstream tools also need to know what native inputs are expected at link time.

## What The Link Surface Contains

`BindingLinkSurface` currently carries:

- `preferred_mode`
- `native_surface_kind`
- `platform_constraints`
- `include_paths`
- `framework_paths`
- `library_paths`
- `libraries`
- `frameworks`
- `artifacts`
- `ordered_inputs`

This deliberately preserves both:

- normalized buckets, such as `libraries`
- original ordering information, via `ordered_inputs`

## Why Ordered Inputs Matter

Link order can be semantically significant, especially with:

- static archives
- mixed object/archive inputs
- linkers that resolve left-to-right

If `bic` only preserved deduplicated buckets, a downstream tool could lose the original intended order and silently produce a different result.

## Declared Libraries

Library-name inputs are recorded with:

- name
- kind
- source

Kinds:

- `Default`
- `Static`
- `Dynamic`

Examples:

```rust
let cfg = HeaderConfig::new()
    .header("api.h")
    .link_lib("z")
    .link_static_lib("foo")
    .link_shared_lib("dl");
```

## Concrete Artifacts

When the binding surface depends on explicit files instead of library names, use artifact inputs:

- `link_object_file(...)`
- `link_static_artifact(...)`
- `link_shared_artifact(...)`

Each artifact preserves:

- `path`
- `kind`
- `source`

This is important for vendored or generated native inputs that are not discoverable through a generic `-lfoo` model.

## Framework Inputs

For Apple-style native dependencies:

- `framework_dir(...)`
- `link_framework(...)`

These are preserved separately from ordinary library names because they are resolved differently by downstream toolchains.

## Preferred Link Mode

`preferred_mode` captures the scan-time preference between:

- `Default`
- `PreferStatic`
- `PreferDynamic`

This is not the same as hard pinning every input.
It is a policy hint attached to the package.

Use:

```rust
.prefer_static_linking()
```

or:

```rust
.prefer_dynamic_linking()
```

when the package should carry that preference explicitly.

## Native Surface Kind

`native_surface_kind` classifies the package at a higher level:

- `HeaderOnly`
- `LibraryNames`
- `ConcreteArtifacts`
- `Mixed`

This gives downstream consumers a quick decision point.

Examples:

- pure header extraction with no native requirements -> `HeaderOnly`
- only `link_lib("sqlite3")` -> `LibraryNames`
- only explicit `.a` / `.so` / `.o` inputs -> `ConcreteArtifacts`
- any mix of library names and explicit files/frameworks -> `Mixed`

## Requirement Provenance

Link requirements preserve a `source`:

- `Declared`
- `Inferred`
- `Discovered`

That distinction matters because downstream tooling often wants to trust user declarations more than inferred guesses, while still preserving discovered evidence for reporting and future planning.

## Platform Constraints

`platform_constraints` are package-level target applicability hints.

Today they are strings rather than a rich constraint language.
That still makes them useful for:

- simple target gating
- downstream filtering
- build-graph selection

## Reading The Link Surface Programmatically

The link surface is already part of `BindingPackage`, so most consumers should read it directly:

```rust
let package = HeaderConfig::new()
    .header("api.h")
    .link_lib("demo")
    .process()?
    .package;

let link = &package.link;
println!("ordered inputs: {}", link.ordered_inputs.len());
```

This keeps link-planning policy in the downstream library or tool that consumes `bic`.

## Normalized Plan Artifact

When a consumer wants a library-facing planning artifact instead of reading raw link buckets
directly, it can call:

```rust
let plan = bic::resolve_link_plan(&package);
assert_eq!(plan.inputs.len(), package.link.ordered_inputs.len());
```

`ResolvedLinkPlan` is intentionally still a normalized metadata artifact.
It is not yet a full filesystem-resolved linker invocation.
