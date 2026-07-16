# Release policy and checklist

The root `RELEASE.md`, which is included in the package archive, is the
normative distribution policy. This chapter summarizes the repository checks.

## Identity and compatibility

The current identities are:

- package `follang-linc` 0.1.0, imported as `linc`;
- MSRV Rust 1.89;
- link-analysis schema `follang.linc.link-analysis` version 2;
- H5 implementation baseline
  `4a2e0fba3aa528b7ba55626fd1ea9c75d153f7b1`; and
- exact upstream `follang-parc` 0.16.0 revision
  `ba603cdccc9375473eca0c42e5462cf90b6da249`.

Registry publication is disabled. The project does not claim crates.io name
ownership or availability. Distribution uses an exact Git tag and its tested
self-contained Cargo archive.

Rust API and behavior changes follow SemVer. Before 1.0, breaking changes
require a minor bump. Frozen schema v2 is never changed in place: incompatible
artifact shape, meaning, canonicalization, or fingerprint changes require a
new schema and corpus plus a breaking SemVer bump. A patch release does not
raise the MSRV. Detailed rules are in `RELEASE.md`.

## Certified boundary

The production certifier covers the explicitly checked C17 GNU x86-64 Linux
ELF LP64/SysV profile with GCC or Clang, exact native inputs, measured layouts,
and the source-type subset accepted by `NativeAnalyzer::certify`. Unsupported
targets, ABIs, source types, provider states, and compiler observations fail at
the owning boundary. No distribution metadata broadens that tested matrix.

## Candidate gate

The operator must first fetch and review the tracked upstream and tags in both
LINC and PARC. On clean branches whose `HEAD`s exactly equal their tracked
upstreams, run:

```sh
make release-check
```

The target refuses detached, dirty, untracked, non-upstream, already-tagged,
registry-publishable, or wrong-PARC state, then runs the full `make verify`
gate. It is non-mutating: it does not fetch, edit a version, commit, tag, push,
upload, or publish.

The full gate proves:

- formatting, Clippy, feature combinations, tests, and doctests;
- frozen schema-v2 corpus preservation;
- required Linux ELF and compiler evidence with no zero-test filter;
- extracted PARC/LINC archives and a clean packaged consumer;
- mdBook and Rust API documentation; and
- no worktree change during verification.

## Dependency order

Record the full PARC tag commit, package version, and source-schema version
before LINC tagging. Then tag LINC against that exact state, GERC against exact
PARC and LINC states, and finally update FOL's lock. Never tag a downstream
crate against uncommitted or local-only upstream state.
