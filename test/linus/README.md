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
