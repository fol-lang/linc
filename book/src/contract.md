# Contract

Use `linc::contract` directly:

```rust
use linc::contract::{decode_link_analysis, ValidatedLinkAnalysis};

# fn check(
#   source: &parc::contract::CompleteSourcePackage,
#   bytes: &[u8],
# ) -> Result<(), Box<dyn std::error::Error>> {
let package = decode_link_analysis(bytes)?;
let validated = ValidatedLinkAnalysis::try_new(source, package)?;
assert_eq!(validated.package().source_fingerprint(), source.source().fingerprint());
# Ok(())
# }
```

`LinkAnalysisPackage` is immutable and constructed through checked APIs. Its
schema-v2 decoder rejects unknown fields, noncanonical collections, forged
identities, incoherent evidence dimensions, and resource-limit violations.
`ValidatedLinkAnalysis` additionally proves exact coverage of a complete PARC
declaration closure.

`LinkAnalysisPackageInput` remains public for corpus decoding and low-level
contract construction. Native consumers must not treat it as the authoritative
inspection boundary: enable `native-inspection` and call
`NativeAnalyzer::certify`, which accepts no caller-authored ABI facts and
returns a `ValidatedLinkAnalysis` only after resolution, measurement, and all
evidence dimensions pass. `NativeAnalyzer::analyze` remains available only as
the advanced typed-evidence intake.

Native inputs and resolved link atoms are sequences: order and repetition are
semantic. Inventories, probes, layouts, declaration evidence, and diagnostics
use their documented canonical order.

Build the contract-only surface with:

```text
cargo check --no-default-features --features contracts
```
