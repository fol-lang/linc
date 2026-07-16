# LINC

LINC turns a complete PARC source package plus native inputs into checked,
source-bound link and ABI evidence for downstream generators.

The default `linc::contract` surface provides immutable schema-v2 packages,
strict resource-bounded encoding/decoding, ordered and repeated link plans,
and `ValidatedLinkAnalysis` coverage proofs. The optional
`native-inspection` feature adds the certified Linux ELF implementation:

- bounded inspection of ELF objects, archives, and shared libraries;
- exact artifact, target, symbol, version, visibility, and `DT_NEEDED`
  evidence using the pinned `object` parser;
- deterministic exact-path or declared-search-path resolution with explicit
  ambiguity, weak-symbol, order, repetition, and transitive-provider rules;
- direct-argv ABI probes with a cleared environment, secure temporary files,
  compiler identity, wall/output bounds, and Linux process-group cleanup; and
- `NativeAnalyzer`, the authoritative operation that resolves, validates every
  selected declaration and evidence dimension, and returns only a
  `ValidatedLinkAnalysis`.

```rust
# #[cfg(feature = "native-inspection")]
# fn build_analyzer() -> Result<(), Box<dyn std::error::Error>> {
use linc::native::{
    InspectionLimits, NativeAnalyzer, NativeInspector, NativeResolver,
    ResolverConfiguration,
};

let inspector = NativeInspector::new(InspectionLimits::default())?;
let resolver = NativeResolver::new(inspector, ResolverConfiguration::default())?;
let analyzer = NativeAnalyzer::new(resolver);
assert_ne!(analyzer.resolver().inspector().limits().max_symbols, 0);
# Ok(())
# }
```

Enable the implementation explicitly:

```toml
[dependencies]
linc = { package = "follang-linc", version = "0.1", default-features = false, features = ["native-inspection"] }
```

The certified H3 platform tier is Linux ELF. Mach-O, COFF/import libraries,
frameworks, and ambient loader/linker lookup are not silently accepted. A
foreign-target probe must be compile-only or use an explicit absolute runner.
Memory/process-count fields remain recorded contract evidence; the H3 runner
enforces wall time, captured output, bounded file reads, and descendant process
cleanup rather than claiming a portable OS memory sandbox.

Run the real native-evidence lane with `make test-native`, or all repository
gates with `make verify`.

The packaged preservation pair remains available under
`linc::contract::corpus` for H1 contract compatibility.
