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

## Reading The Link Surface From JSON

The CLI makes link extraction easy:

```sh
bic link-plan --bindings-json bindings.json
```

That command prints the normalized `BindingLinkSurface` only.

This is useful when:

- you want to inspect link metadata without the full declaration payload
- another tool only cares about native inputs
- you want a compact snapshot in tests
