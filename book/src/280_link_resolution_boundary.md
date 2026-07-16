# Link Resolution Boundary

This chapter defines the boundary between LINC link metadata and downstream
build-system work.

## What LINC Resolves Today

LINC preserves declared native link intent, normalized native link metadata,
ordered inputs, requirement provenance, platform hints, symbol inventories, and
validation evidence.

## What LINC Does Not Resolve Today

LINC does not promise final linker invocation, full search-path expansion, or
runtime loader behavior.

`ResolvedLinkPlan` associates declared requirements with supplied candidate
inventories. A one-candidate `Resolved` state is not filesystem, provider,
linker, loader, or deployment truth.

## Practical Rule For Consumers

Treat `BindingPackage.link` as normalized requirement metadata and keep final
linker invocation in downstream tooling.
