# Unsupported Cases

This chapter records the important unsupported or incomplete cases explicitly.

The purpose is to prevent downstream users from mistaking absence of detail for implicit support.

## Native Artifact Formats

Currently unsupported or incomplete at a production-ready level:

- Windows-native COFF/PE artifact inspection
- import-library parity with ELF archive/shared-library workflows
- fully modeled forwarded/re-exported symbol cases across all platforms

## ABI Modeling

Currently incomplete:

- bitfield representation as a first-class stable contract
- field-offset coverage directly attached to extracted record fields
- enum underlying representation as a complete, stable contract
- calling conventions beyond the current baseline `C` model
- per-declaration ABI-confidence metadata

## Macro Semantics

Currently incomplete:

- stable lowering for arbitrary function-like macros
- source-location/provenance for all macro captures
- fully explicit effective macro-environment reporting

## Validation Depth

Currently incomplete:

- ABI-shape validation beyond symbol presence/provenance-oriented checks
- strong Windows-native linker/provider modeling
- fully modeled re-export and alias evidence across every artifact format

## Consumer Guidance

Downstream tools should treat these gaps as explicit non-guarantees.

That means:

- do not build hard production assumptions on these areas yet
- isolate experimental handling behind clear policy gates
- prefer documented Tier 1 surfaces when designing the main integration path

## Why This Chapter Exists

A production-oriented library needs explicit unsupported-case documentation.

Otherwise users will accidentally rely on:

- behavior that merely happens to work on one platform
- incomplete representations that look more complete than they are
- future roadmap items that have not actually landed
