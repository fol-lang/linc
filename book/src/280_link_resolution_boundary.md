# Link Resolution Boundary

This chapter defines the boundary between LINC link metadata and downstream build-system work.

That boundary matters because LINC is intentionally a library for analysis and normalized link
intent, not a replacement for a platform linker, package manager, or full build graph.

## What LINC Resolves Today

LINC currently resolves and preserves:

- declared native link intent from headers/configuration
- normalized library/framework/artifact metadata
- ordered native inputs
- requirement provenance such as declared, inferred, or discovered
- platform applicability hints
- symbol inventories from supported native artifacts
- validation evidence about whether declarations appear to have native providers

## What LINC Does Not Resolve Today

LINC does not currently promise:

- final filesystem search resolution for `-lfoo` style inputs
- system-specific linker search path expansion
- package-manager discovery of native dependencies
- full transitive native dependency closure
- platform linker flag synthesis for every toolchain
- final duplicate/conflict resolution across the full build graph
- runtime loader behavior

## Practical Rule For Consumers

The safe rule is:

- treat `BindingPackage.link` as normalized requirement metadata
- treat symbol inspection and validation as evidence
- treat final linker invocation and native dependency resolution as downstream responsibilities

## Why This Split Is Intentional

Keeping this boundary explicit makes the library more reusable.

Different downstream consumers may:

- generate bindings only
- produce build metadata for another tool
- integrate with an existing package manager
- perform stricter resolution for one platform than another

If LINC claimed to fully resolve all of that itself, it would either become much less general or
would make promises it cannot yet uphold consistently.

## What A Downstream Tool Should Do

A serious downstream consumer should usually:

1. read the normalized link surface from `BindingPackage.link`
2. apply its own target selection and policy gates
3. resolve library names to actual native artifacts in its own environment
4. optionally compare the resolved artifacts with LINC symbol inventories and validation reports
5. assemble the final linker or build-system invocation itself

## `fol` As An Example Consumer

`fol` is one example consumer of this boundary.

It may rely on LINC for extraction, normalization, and evidence gathering, while still keeping
its final code-generation and native-link orchestration policy on the consumer side.

That example should not be read as a special-case contract.
It is one integration profile built on top of the same generic library boundary.
