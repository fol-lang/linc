# Link Surface

`BindingPackage.link` is the normalized native-link surface attached to a scan.

This is one of the most important additions in the current LINC architecture because binding generation alone is not enough.
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

If LINC only preserved deduplicated buckets, a downstream tool could lose the original intended order and silently produce a different result.

## Declared Libraries

Library-name inputs are recorded with:

- name
- kind
- source

Kinds:

- `Default`
- `Static`
- `Dynamic`

Provider matching for declared library names is intentionally tolerant of ordinary platform naming
shapes. Today that includes:

- `lib{name}.a`
- `lib{name}.so`
- `lib{name}.so.*`
- `lib{name}.dylib`
- `lib{name}.*.dylib`
- `lib{name}.tbd`
- `lib{name}.*.tbd`

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

This keeps link-planning policy in the downstream library or tool that consumes LINC.

## Normalized Plan Artifact

When a consumer wants a library-facing planning artifact instead of reading raw link buckets
directly, it can call:

```rust
let plan = bic::resolve_link_plan(&package);
assert_eq!(plan.inputs.len(), package.link.ordered_inputs.len());
```

`ResolvedLinkPlan` is intentionally still a normalized metadata artifact.
It is not yet a full filesystem-resolved linker invocation.

When inventories are available, consumers can also separate declared requirements from candidate
providers:

```rust
let plan = bic::resolve_link_plan_with_inventories(&package, &inventories);
for requirement in &plan.requirements {
    println!("{:?}: {}", requirement.resolution, requirement.providers.len());
}
```

That keeps "what the package asked for" distinct from "what the current artifact set appears to
provide".

If a consumer is planning for one concrete target, it can also filter the plan by target triple:

```rust
let linux_plan = bic::resolve_link_plan_for_target(
    &package,
    &inventories,
    Some("x86_64-unknown-linux-gnu"),
);
```

Today this uses simple substring matching over `platform_constraints`.
That should be read as target-applicability filtering, not as a full constraint language.

The requirement resolution state is explicit:

- `Unresolved`
- `Resolved`
- `Ambiguous`

When providers come from inspected shared libraries, their dependency edges are also preserved in
the plan so downstream tooling can see the current known transitive native surface without losing
the distinction between declared requirements and discovered dependency evidence.

That means a planning inventory can legitimately resolve against macOS text stubs such as
`/usr/lib/libSystem.tbd` even when later runtime or deployment policy is handled somewhere else.

Requirement and provider provenance are also preserved explicitly:

- requirement source stays attached from the declared package metadata
- provider provenance distinguishes exact declared-artifact matches from discovered inventory-based
  matches
