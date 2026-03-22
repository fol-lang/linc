# Platform Support

This chapter records the current practical platform-support posture of LINC.

## Current Matrix

| Area | Linux / ELF | Apple / Mach-O | Windows / COFF |
|---|---|---|---|
| header scanning | usable | usable with Apple-specific link metadata support | limited by missing Windows-native completion work |
| macro capture | usable | usable | compiler-dependent and not yet fully characterized |
| layout probing | usable with GCC/Clang-style toolchains | usable with Clang-style toolchains | not yet a completed support target |
| symbol inventory | usable | partial but present | present for COFF objects, import libraries, and PE binaries, but still not production-ready |
| validation | usable where symbol inventory is usable | partial | limited; supported inventory classes are tested but the Windows linker model is still incomplete |
| link-surface metadata | usable | usable, including frameworks | partial, especially around Windows-native link forms |

## What "Usable" Means Here

Usable means the feature exists, it has direct test coverage in this
repository, and it is reasonable for controlled internal use.

## Recommended Production Posture

- prefer Linux/ELF for the most mature end-to-end native validation path
- treat Apple support as useful but still maturing
- treat Windows-native linker/artifact support as incomplete

For the Level 1 production claim, interpret that as:

- Linux/ELF = primary production platform
- Apple/Mach-O = secondary confidence scope
- Windows/COFF = tested but non-primary confidence scope
