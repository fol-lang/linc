# Distribution and release policy

This file defines LINC's distribution identity and compatibility rules. It is
included in every Cargo archive.

## Current identity

| Item | Value |
| --- | --- |
| Cargo package | `follang-linc` 0.1.0 |
| Rust library/import | `linc` |
| Edition | Rust 2021 |
| MSRV | Rust 1.89 |
| License | `MIT OR Apache-2.0` |
| Link-analysis schema | `follang.linc.link-analysis`, version 2 |
| Certified implementation surface | H5 Linux ELF certification |
| H5 implementation baseline | `4a2e0fba3aa528b7ba55626fd1ea9c75d153f7b1` |
| Required PARC package | `follang-parc` exactly 0.16.0 |
| Required PARC revision | `11ca2be6d3dcda7227c0d9eb6c90259838f289fc` |

The Rust constants `LINK_ANALYSIS_SCHEMA_ID` and
`LINK_ANALYSIS_SCHEMA_VERSION` are the authority for artifact consumers. The
H5 baseline identifies the implementation certified before this
distribution-only hardening change. A release tag records the exact
archive-producing commit, including later documentation or packaging changes.

The production certification surface is the explicitly checked C17 GNU
x86-64 Linux ELF LP64 profile with GCC or Clang, exact native inputs, and the
supported source-type subset enforced by `NativeAnalyzer::certify`. LINC does
not claim certified Mach-O, COFF/import-library, framework, non-SysV ABI,
arbitrary C extension, ambient linker-search, or downstream Rust-generation
support. `NativeAnalyzer::analyze` remains an advanced typed-evidence intake;
it is not the production certifier.

## Distribution channel

`Cargo.toml` sets `publish = false`. No crates.io name ownership, availability,
or published release is asserted. The supported distribution channel is a
self-contained `.crate` archive produced from an exact Git tag. Consumers use
that archive or the exact tag commit and import the library as `linc`.

`make test-package` builds PARC and LINC candidate archives, unpacks them
outside both repositories, runs LINC's packaged contract and Linux native test
surfaces, and builds/tests a clean external consumer against package
`follang-linc` under the crate name `linc`. The external consumer selects the
extracted package identities by exact version; it does not depend on repository
source paths or path-only development dependencies.

The default `contracts` feature exposes the durable schema and validation
types. `native-inspection` adds the Linux ELF inspector, resolver, bounded
compiler observation, and production certifier. The archive includes the
fixtures required by both promised test surfaces.

## Compatibility versions

The Cargo package version follows SemVer for the Rust API and documented
behavior. Before 1.0, a breaking Rust API or behavior change requires a minor
version bump; a backwards-compatible fix or additive change may use a patch
bump. After 1.0, normal SemVer major/minor/patch rules apply.

The link-analysis schema is an independent compatibility axis:

- Schema v2 is frozen: it is never changed in place. Any incompatible emitted
  shape, canonical ordering, fingerprint input, or semantic change requires a
  new link-analysis schema version, a new frozen corpus, and a breaking SemVer
  bump (minor before 1.0, major after 1.0).
- Compatible implementation fixes that leave canonical bytes, schema meaning,
  and fingerprints unchanged do not bump the schema version.
- Additive decoder support for an additional, explicitly implemented schema
  may use a compatible SemVer bump only when the existing encoder, decoder,
  and schema-v2 behavior remain unchanged.
- Decoders accept only versions they explicitly implement. A package version
  bump never makes an unknown schema acceptable.

Changing the certified target or supported-type matrix must update tests and
documentation in the same change. Broadening that matrix is not evidence that
an untested platform or ABI is certified.

The MSRV is the `package.rust-version` value in `Cargo.toml` and is exercised by
CI. A patch release does not raise it. Before 1.0, raising the MSRV requires at
least a minor package-version bump; after 1.0 it requires at least a minor bump.
The change must update `Cargo.toml`, this file, and CI together.

## Release order and clean-upstream rule

Sibling releases or tags are ordered:

1. PARC contract archive/tag.
2. LINC against that exact PARC version and commit.
3. GERC against those exact PARC and LINC versions and commits.
4. FOL after its lock records all three exact revisions.

Never tag LINC against uncommitted or merely local PARC state.

Before proposing `follang-linc-v<version>`:

1. merge the candidate and its required PARC revision to their tracked
   upstream branches;
2. run `git fetch --tags origin` in both repositories and review the fetched
   state;
3. check out both release branches with clean worktrees;
4. run `make release-check` from LINC;
5. review the reported version, tag name, full LINC commit ID, and full PARC
   commit ID;
6. create the tag/archive manually under the repository's review policy;
7. record that exact LINC tag commit, package version, schema version, and PARC
   dependency in GERC before any GERC tag.

`make release-check` refuses detached, dirty, untracked, non-upstream, already
tagged, registry-publishable, or wrong-PARC state. It then runs `make verify`.
It performs no fetch, version edit, commit, tag, push, upload, or publication.
