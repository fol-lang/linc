# Platform Support

This chapter records current platform evidence. It is not a production support
or certification matrix.

## Current Matrix

| Area | Linux / ELF evidence | Apple / Mach-O evidence | Windows / COFF evidence |
|---|---|---|---|
| header/bootstrap paths | Linux system fixtures require installed compiler and headers | configuration/fixture paths only | configuration/fixture paths only |
| layout probing | GCC/Clang-driven host tests and fixtures | no native H0 CI gate | no native H0 CI gate |
| symbol inventory | ELF fixtures plus required Linux system lanes | controlled/synthetic Mach-O fixtures | controlled/synthetic COFF/import-library/PE fixtures |
| validation | supplied-inventory symbol and optional shape comparisons | same model on controlled fixtures | same model on controlled fixtures |
| link metadata | library/artifact inputs | framework metadata fixtures | Windows link-form fixtures |

## Interpretation

Linux has the strongest current evidence because required system tests run
there, but H0 is still a verification foundation rather than a production
certification. Apple and Windows rows describe code paths and controlled
fixtures only. Neither has native CI evidence, so neither may be advertised as
a supported production tier.
