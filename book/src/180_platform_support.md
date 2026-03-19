# Platform Support

This chapter records the current practical platform-support posture of `bic`.

The important point is that "parses some C" and "production-ready across native platforms" are
not the same claim.

## Current Matrix

| Area | Linux / ELF | Apple / Mach-O | Windows / COFF |
|---|---|---|---|
| header scanning | usable | usable with Apple-specific link metadata support | limited by missing Windows-native completion work |
| macro capture | usable | usable | mostly compiler-dependent, not yet fully characterized |
| layout probing | usable with GCC/Clang-style toolchains | usable with Clang-style toolchains | not yet a completed support target |
| symbol inventory | usable | partial but present | not production-ready |
| validation | usable where symbol inventory is usable | partial | not production-ready |
| link-surface metadata | usable | usable, including frameworks | partial, especially around Windows-native link forms |

## What "Usable" Means Here

In this chapter, usable means:

- the feature exists
- it has direct test coverage in this repository
- it is reasonable for controlled internal use

It does not automatically mean:

- ABI completeness for arbitrary third-party libraries
- long-term semver confidence for every edge case
- parity with every compiler and linker variant on that platform

## Linux / ELF

Linux/ELF is currently the strongest native-artifact path in the library.

That includes:

- object/archive/shared-library inspection
- dependency-edge capture from shared libraries
- validation against discovered symbols
- direct matrix tests for ELF object, static-library, and shared-library format/capability expectations

For current production-oriented internal use, ELF should be treated as the primary supported
native-artifact environment.

## Apple / Mach-O

Mach-O support exists and is useful, but it is still behind ELF in overall maturity.

Current strengths:

- Mach-O symbol parsing exists
- framework metadata is modeled in the link surface
- Apple-specific scan metadata can be preserved
- direct matrix tests cover Mach-O object, static-library, and dylib format/capability expectations

Current caveats:

- validation depth is not yet as battle-tested as ELF
- platform-specific linking behavior is still less fully modeled than it needs to be for a strong
  "full binder/linker" claim

## Windows / COFF

Windows-native artifact support should currently be read as incomplete.

That means downstream consumers should not yet assume:

- import-library inspection parity
- robust decoration handling across Windows-native conventions
- production-ready validation against the Windows linker model

This is a roadmap gap, not just a documentation gap.

## Recommended Production Posture

For now:

- prefer Linux/ELF for the most mature end-to-end native validation path
- treat Apple support as useful but still maturing
- treat Windows-native linker/artifact support as incomplete

If another tool depends on `bic` in a multi-platform release flow, that tool should encode these
platform expectations explicitly rather than assuming uniform maturity.
