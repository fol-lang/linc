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
  compiler identity, wall/output bounds, and Linux process-group cleanup;
- bounded `CertificationToolchain::observe`, which owns compiler identity
  observation before PARC target construction; and
- `NativeAnalyzer::certify`, the production operation that resolves providers,
  generates header-free structural probes, measures layouts, certifies SysV64
  call shapes against compiled witnesses, and returns only a
  `ValidatedLinkAnalysis`. The lower-level `analyze` API is retained for
  advanced evidence producers.

```rust
# #[cfg(feature = "native-inspection")]
# fn build_analyzer() -> Result<(), Box<dyn std::error::Error>> {
use linc::native::{
    CertificationToolchain, InspectionLimits, NativeAnalyzer, NativeInspector,
    NativeResolver, ResolverConfiguration,
};
use linc::contract::ProbeResourceLimits;
use std::path::PathBuf;

let inspector = NativeInspector::new(InspectionLimits::default())?;
let resolver = NativeResolver::new(inspector, ResolverConfiguration::default())?;
let analyzer = NativeAnalyzer::new(resolver);
assert_ne!(analyzer.resolver().inspector().limits().max_symbols, 0);
let limits = ProbeResourceLimits::try_new(10_000, 512 << 20, 1 << 20, 16)?;
# let compiler = PathBuf::from("/absolute/path/to/cc");
# if compiler.exists() {
let toolchain = CertificationToolchain::observe(compiler, Vec::new(), limits)?;
assert!(!toolchain.reported_target().is_empty());
# }
# Ok(())
# }
```

For a repository development checkout, spell both package and library
identities explicitly:

```toml
[dependencies]
linc = { package = "follang-linc", path = "../linc", default-features = false, features = ["native-inspection"] }
```

Registry publication is disabled. Released consumers must use the exact tested
Git tag/archive described by the release policy rather than inventing a
registry version or following an unpinned branch.

The initial certified platform tier is C17 GNU x86-64 Linux ELF LP64 with the
SysV ABI and GCC or Clang. Mach-O, COFF/import libraries, frameworks, and
ambient loader/linker lookup are not silently accepted. A foreign-target probe
must be compile-only or use an explicit absolute runner.
Memory/process-count fields remain recorded contract evidence; the Linux
runner enforces wall time, captured output, bounded file reads, and descendant
process cleanup rather than claiming a portable OS memory sandbox.

Run the real native-evidence lane with `make test-native`, or all repository
gates with `make verify`.

The packaged preservation pair remains available under
`linc::contract::corpus` for schema-v2 compatibility.

## Distribution and compatibility

The package identity is `follang-linc` 0.1.0 and the Rust import name is
`linc`. Registry publication is disabled (`publish = false`), so no crates.io
name ownership or availability is claimed. Candidate archives are tested with
the exact `follang-parc` 0.16.0 contract at revision
`0f52aeeeeec47a082c0d8a515130ee853aa1101d` and with a clean external consumer.

`make release-check` is a non-mutating eligibility check. It never changes a
version, commits, tags, pushes, uploads, or publishes. SemVer, schema-v2, MSRV,
certified-surface, exact-upstream, and tag/archive rules are recorded in
[`RELEASE.md`](RELEASE.md).

## License

Dual-licensed under Apache 2.0 or MIT.
