# Hardening Status

This book documents the current H0 baseline. LINC is not production-certified,
the PARC/LINC/GERC pipeline is not yet certified for FOL V4, and the H1 through
H5 contracts described in the hardening plan are not implemented milestones.

## Identity And Toolchain

| Item | Current value |
|---|---|
| Distribution package | `follang-linc` |
| Rust library/import name | `linc` |
| Declared MSRV | Rust 1.89 |
| Registry publication | Deferred to the H6 distribution gate |
| Evidence artifact schema | Version 1; not the frozen H1 schema |

The package and library names are intentionally different. Cargo dependency
metadata uses `follang-linc`; Rust code imports `linc`.

## Evidence Semantics

| Surface | Current meaning | Not proved |
|---|---|---|
| `ResolvedLinkPlan` | Normalized requirements plus zero, one, or multiple filename/path-matched candidate inventories | Filesystem search closure, final linker arguments, link success, or runtime loading |
| `RequirementResolution::Resolved` | Exactly one candidate inventory matched | Provider authenticity, target compatibility, or availability on another host |
| `ValidationReport::Matched` | A visible exported symbol with the expected function/variable kind matched by name | Full ABI compatibility |
| Optional ABI-shape evidence | Available size/count/shape observations were compared | Calling conventions, aggregate passing, variadics, full target ABI, or behavioral compatibility |
| Apple/Windows fixtures | Format parsing and controlled/synthetic inventory evidence | Native platform certification; H0 has no native Apple or Windows CI gate |

Validation reports are structured evidence for downstream policy. Their names
must not be read as linker, loader, provider, or ABI certification.

## Verification Interface

| Command | Purpose | Prerequisites |
|---|---|---|
| `make build` | Release build | Rust 1.89 toolchain |
| `make fmt-check` | Rust formatting check | `rustfmt` |
| `make lint` | Clippy with warnings denied | `clippy` |
| `make check-features` | Default, all-feature, and no-default checks | Cargo |
| `make test` | Hermetic required tests and doctests | Cargo |
| `make test-contract` | Artifact and public-API contract tests | Cargo |
| `make test-package` | Package archive and clean-consumer check | Cargo and the repository script |
| `make test-system` | Ignored compiler/tool/header/library-dependent tests | Every prerequisite required by the selected system fixtures |
| `make docs-check` | mdBook and Rust API docs | `mdbook`, Cargo/rustdoc |
| `make verify` | Full non-mutating gate | All required prerequisites and a clean worktree |

`make test-system` is a required lane: missing prerequisites fail instead of
silently skipping. Required CI installs them. Documentation builds write under
`target/` and never stage or commit files.
