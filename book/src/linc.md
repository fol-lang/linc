# LINC

LINC is the checked native-link and ABI-evidence boundary between PARC source
packages and downstream generators.

The durable API is `linc::contract`. It accepts a complete PARC source closure,
preserves native-input and resolved-link order, and carries the symbol, layout,
callable-ABI, probe, policy, and diagnostic evidence needed to validate that
closure.

With the `native-inspection` feature, `linc::native` implements the initial
certified Linux ELF lane. `CertificationToolchain::observe` owns bounded
compiler observation and `NativeAnalyzer::certify` owns resolution, structural
measurement, strict declaration validation, package construction, and final
source validation. Production consumers should use those operations instead
of assembling native evidence. `NativeAnalyzer::analyze` is the advanced
lower-level intake.

PARC owns source semantics, LINC owns schema-v2 link analysis, and generation
belongs downstream.

The preservation corpus continues to establish the schema-v2 contract pair.
The native lane certifies LINC's own source-bound link and ABI-evidence
responsibilities; complete PARC -> LINC -> GERC certification remains a system
concern.
