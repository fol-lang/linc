# Link Surface

`BindingPackage.link` is the normalized native-link surface attached to a
scan.

This is one of the most important pieces of LINC because evidence generation
alone is not enough. Downstream tools also need to know what native inputs are
expected at link time.

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

This deliberately preserves both normalized buckets, such as `libraries`, and
original ordering information, via `ordered_inputs`.

## Why Ordered Inputs Matter

Link order can be semantically significant, especially with static archives,
mixed object/archive inputs, and linkers that resolve left-to-right.

If LINC only preserved deduplicated buckets, a downstream tool could lose the
original intended order and silently produce a different result.

## Declared Libraries

Library-name inputs are recorded with a name, a kind, and a source.

Kinds:

- `Default`
- `Static`
- `Dynamic`

Provider matching for declared library names is intentionally tolerant of
ordinary platform naming shapes.

This is filename-shape matching against supplied inventories, not a filesystem
search or provider-identity proof.

## Concrete Artifacts

When the binding surface depends on explicit files instead of library names,
use artifact inputs.

Each artifact preserves path, kind, and source. That is important for vendored
or generated native inputs that are not discoverable through a generic
`-lfoo` model.

## Framework Inputs

For Apple-style native dependencies, frameworks are preserved separately from
ordinary library names because they are resolved differently by downstream
toolchains.

## Preferred Link Mode

`preferred_mode` captures the scan-time preference between default, preferred
static, and preferred dynamic.

This is not the same as hard pinning every input. It is a policy hint attached
to the package.

## Native Surface Kind

`native_surface_kind` classifies the package at a higher level:

- `HeaderOnly`
- `LibraryNames`
- `ConcreteArtifacts`
- `Mixed`

This gives downstream consumers a quick decision point.

## Requirement Provenance

Link requirements preserve a source:

- `Declared`
- `Inferred`
- `Discovered`

That distinction matters because downstream tooling often wants to trust user
declarations more than inferred guesses while still preserving discovered
evidence for reporting and future planning.

## Platform Constraints

`platform_constraints` are package-level target applicability hints.

Today they are strings rather than a rich constraint language. That still makes
them useful for simple target gating, downstream filtering, and build-graph
selection.

## Reading The Link Surface Programmatically

Most consumers should read the link surface directly from `BindingPackage`.
That keeps link-planning policy in the downstream library or tool that consumes
LINC.

## Normalized Plan Artifact

`ResolvedLinkPlan` is the normalized planning artifact. It is intentionally not
a full filesystem-resolved linker invocation.

When inventories are available, consumers can separate declared requirements
from candidate providers.
- `Unresolved`
- `Resolved`
- `Ambiguous`

These states count matching candidate inventories: zero, one, or more than
one. `Resolved` does not mean that a path exists on the consumer's host, that
the candidate is target-compatible, that linking succeeds, or that a runtime
loader can find it.

When providers come from inspected shared libraries, their dependency edges are also preserved in
the plan so downstream tooling can see the current known transitive native surface without losing
the distinction between declared requirements and discovered dependency evidence.

For example, a controlled inventory can associate a requirement with a macOS
text-stub path such as `/usr/lib/libSystem.tbd`. That remains fixture or
supplied-inventory evidence; H0 does not certify native Apple resolution.

Requirement and provider provenance are also preserved explicitly:

- requirement source stays attached from the declared package metadata
- provider provenance distinguishes exact declared-artifact matches from
  discovered inventory-based candidate matches; it does not authenticate the
  provider
