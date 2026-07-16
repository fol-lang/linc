# LINC

LINC is the checked native-link and ABI-evidence boundary between PARC source
packages and downstream generators.

The sole public surface is `linc::contract`. It provides:

- immutable schema-v2 `LinkAnalysisPackage` values;
- strict, resource-bounded encoding and decoding;
- lossless native inputs and ordered, repeated resolved link plans;
- symbol, layout, callable-ABI, probe, policy, and diagnostic evidence;
- `ValidatedLinkAnalysis`, proving exact coverage of a complete PARC closure.

```rust
use linc::contract::{decode_link_analysis, ValidatedLinkAnalysis};

# fn check(
#     source: &parc::contract::CompleteSourcePackage,
#     bytes: &[u8],
# ) -> Result<(), Box<dyn std::error::Error>> {
let package = decode_link_analysis(bytes)?;
let validated = ValidatedLinkAnalysis::try_new(source, package)?;
assert_eq!(validated.package().source_fingerprint(), source.source().fingerprint());
# Ok(())
# }
```

Contract-only consumers can disable defaults explicitly:

```toml
[dependencies]
linc = { package = "follang-linc", version = "0.1", default-features = false, features = ["contracts"] }
```

The packaged preservation pair is available under `linc::contract::corpus`.
It is checked against PARC's complete preservation source artifact by the
package and integration gates.

This crate establishes the LINC side of the H1 contract. It does not by itself
certify the complete PARC -> LINC -> GERC pipeline or later milestones.
