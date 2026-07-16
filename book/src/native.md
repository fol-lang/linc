# Native evidence

Enable `native-inspection` for LINC's certified H3 Linux ELF lane. The lane
accepts ELF relocatable objects, static archives, and shared libraries. It does
not infer support for Mach-O, COFF/import libraries, or frameworks.

## Authoritative operation

Construct an `AnalysisRequest` from a complete PARC closure, an explicit
`AnalysisPolicy`, and ordered `NativeInput` values. Then pass typed probe,
layout, and linked-declaration requests to `NativeAnalyzer::analyze`.

The analyzer:

1. checks source and target fingerprints on all explicit evidence;
2. resolves and inspects every provider without ambient loader lookup;
3. validates provider, symbol, layout, and callable-ABI dimensions separately;
4. requires exact evidence for every selected closure member;
5. constructs the durable schema-v2 package; and
6. returns only after `ValidatedLinkAnalysis::try_new` succeeds.

This prevents downstream generators from constructing apparently valid native
packages from uninspected or partially checked facts.

## Inspection and resolution

Inspection records the canonical path and digest, artifact kind and format,
machine/architecture, pointer width, endianness, requested and observed target,
parser identity, raw and normalized symbol spellings, direction, kind,
binding, visibility, decoration, version, size/address/section/member, and
ordered dynamic dependencies.

Resolution preserves native-input order, repetition, groups, and object
placement. Exact-path mode can bind a transitive `DT_NEEDED` name only to an
already explicit provider identity (canonical filename, explicit alias, or
`DT_SONAME`); it never performs ambient lookup. Search mode consults only the
declared roots, chooses the configured static/dynamic kind before checking
same-name ambiguity, and rejects the same candidate name found in distinct
roots. Dependency providers must follow their parent without reordering the
explicit plan.

ELF versioned symbols, local/hidden/imported symbols, duplicate providers, and
weak providers retain distinct evidence. Strict analysis rejects local,
hidden, imported, ambiguous, or policy-forbidden weak providers rather than
guessing.

## ABI probes

`ProbeRunner` renders only checked include names and explicit source, invokes
the compiler and optional runner using direct argument vectors, clears the
inherited environment, and records compiler executable identity, arguments,
sysroot, target, source fingerprint, execution policy, and exact subject
outcomes.

Probe files live in a secure temporary directory under an absolute caller-owned
parent. The Linux lane bounds wall time, captured output, and file reads and
kills/reaps the invocation process group when a limit is exceeded. A child that
keeps inherited output pipes open is rejected. Foreign targets use compile-only
evidence unless an explicit absolute runner is supplied. Contract
memory/process-count values are recorded but are not described as a portable OS
sandbox.

## Verification

`make test-native` builds real ELF fixtures and exercises successful and
negative inspection, resolution, symbol, ABI, stale-evidence, probe-bound, and
cross-target cases. `make verify` also runs feature, schema, package-consumer,
documentation, and H1 corpus gates.
