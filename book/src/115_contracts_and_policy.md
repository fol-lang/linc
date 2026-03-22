# Contracts And Policy

This section groups the durable contract and policy chapters.

Read this section when you need to answer questions such as:

- what parts of the public API are intended to stay stable
- what current JSON artifact policy means in practice
- which failures are operational errors versus report data
- which fields are identity, stable transport, or evolving evidence

These chapters are the narrowest statement of what downstream consumers should rely on.

If you are integrating LINC into another tool, this section matters more than the implementation
details of lower-level modules.

The most important policy rule is architectural:

- `linc/src/**` must not depend on `parc` or `gec`
- cross-package translation belongs in tests/examples/harnesses
- `linc` owns its own internal model and its own evidence artifacts
- there is no shared ABI crate
- there is no backward-compatibility burden for old pipeline shapes
- bootstrap utilities and repo-local shortcuts must never be mistaken for the public contract
