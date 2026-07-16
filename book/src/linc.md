# LINC

LINC is the checked native-link and ABI-evidence boundary between PARC source
packages and downstream generators.

The durable API is `linc::contract`. It accepts a complete PARC source closure,
preserves native-input and resolved-link order, and carries the symbol, layout,
callable-ABI, probe, policy, and diagnostic evidence needed to validate that
closure.

With the `native-inspection` feature, `linc::native` implements the H3 Linux ELF
lane. `NativeAnalyzer::analyze` owns resolution, strict declaration validation,
package construction, and final source validation. Consumers should use that
operation instead of assembling `LinkAnalysisPackageInput` from native facts.

PARC owns source semantics, LINC owns schema-v2 link analysis, and generation
belongs downstream.

The preservation corpus continues to establish the H1 contract pair. The
native lane certifies LINC's H3 responsibilities only; complete PARC -> LINC ->
GERC pipeline certification remains a system concern.
