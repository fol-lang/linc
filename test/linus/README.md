# Linux Code-Driven Examples

This directory carries Linux-focused, code-driven integration examples for `bic`.

These examples are intentionally library-first:

- no sidecar config files
- no CLI assumptions
- no generated manifests

The goal is to show what a downstream consumer such as `fol` would construct directly in Rust.

Current examples:

- `socketcan.rs`: analyze the Linux SocketCAN headers, attach explicit Linux/link metadata, and
  request ABI-sensitive layout probes entirely from code

## Planned Torture Target

The synthetic torture target is meant to concentrate difficult C interop constructs into one
header-level surface so `bic` limitations are easier to observe and classify.

The first version is intended to include:

- typedef chains and alias-mediated records
- anonymous nested structs and unions
- bitfields and packed records
- flexible array members
- opaque forward declarations
- function-pointer callbacks
- variadic functions
- macro constants and ABI-affecting configuration macros
- one or more intentionally unsupported declarations

The purpose is not realism.
The purpose is to force one scan to answer:

- what extracted cleanly
- what extracted partially with diagnostics
- what was represented as unsupported
- what can be layout-probed with high confidence
