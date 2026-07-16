# Symbol Inventories

When the `symbols` feature is enabled, LINC can inspect native artifacts and
produce a `SymbolInventory`.

This is the artifact-side counterpart to the source-side evidence package.

## Why Symbol Inventories Matter

Header extraction tells you what the C surface claims exists. Artifact
inspection tells you what a native file actually exports or imports.

You need both when you want to answer questions such as:

- does this library really provide the declarations I scanned?
- which artifact satisfies a symbol?
- is the symbol hidden, weak, or duplicated?
- what shared-library dependencies does this artifact declare?

## Entry Point

```rust
use linc::inspect_symbols;

let inventory = inspect_symbols("build/libdemo.so").unwrap();
```

## Supported Artifact Shapes

Current artifact coverage includes:

| Platform format | Typical files | Kinds |
|---|---|---|
| ELF | `.o`, `.a`, `.so` | object, static library, shared library |
| Mach-O | `.o`, `.a`, `.dylib` | object, static library, dynamic library |

The inventory also classifies the artifact at a higher level.

Current metadata includes:

- `format`
- `platform`
- `kind`
- `capabilities`
- `dependency_edges`
- `symbols`

## Artifact Capabilities

`capabilities` currently capture whether an artifact exports symbols or imports
symbols.

That distinction matters for differentiating linkable providers from
dependency-only inputs.

## Symbol Entries

Each `SymbolEntry` carries:

- normalized `name`
- optional `raw_name`
- `direction`
- `visibility`
- whether it is a function or variable-like symbol
- `binding`
- optional `size`
- optional `section`
- optional `archive_member`
- optional `reexported_via`
- optional `alias_of`

### Normalized vs Raw Name

The normalized name is used for matching declarations. The raw name preserves
the original artifact spelling.

`direction` is also important: only exported symbols are candidate providers
during validation. Imported symbols are still preserved because they matter for
shared-library and link-planning analysis.

`alias_of` is preserved when LINC can see more than one exported symbol name
resolving to the same section or address identity.

## ELF Symbol Versions

On ELF artifacts, `SymbolEntry.version` preserves symbol-version evidence when
the object reader can see it.

Downstream consumers should read that evidence conservatively:

- version presence is useful provider metadata
- version absence is not proof that the symbol is unversioned everywhere
- version equality helps distinguish exports that share a base symbol name
- version differences should be treated as a reason to avoid collapsing
  providers too aggressively

LINC does not implement a full ELF linker/version-script resolver. It keeps the
version strings as evidence and leaves policy to downstream consumers.

## Archive Member Provenance

For static libraries, LINC preserves the member path or name that provided each
symbol when available.

That lets downstream validation report a provider more precisely than just the
archive path.

## Shared-Library Dependency Edges

On ELF shared libraries and executables, LINC captures `DT_NEEDED`
dependencies into `dependency_edges`.

This is not a full dynamic-loader model. It is still useful because it exposes
artifact-declared native dependencies in the inventory itself.

When LINC sees imported symbols inside a shared library or executable, it also
preserves symbol-local `reexported_via` evidence using those dependency edges.

## Platform Behavior Notes

Mach-O commonly prefixes external symbols with `_`.
LINC normalizes those names so C declarations and native symbols compare more
naturally.

That normalization is intentionally paired with `raw_name` preservation so no
spelling evidence is lost.

Mach-O support should still be read conservatively:

- imported symbols are useful dependency evidence, not proof of final loader
  behavior
- re-export inferences are narrower than a full dyld model
- framework and install-name semantics remain downstream policy concerns
- normalized names are for matching, while `raw_name` stays the authoritative
  original spelling

## Mach-O Limits And Conservative Provider Policy

Downstream consumers must treat Mach-O rows as controlled inventory evidence,
not native platform certification or provider truth. H0 has no native Apple CI,
and Mach-O linking/loading semantics require more context than a plain symbol
table can prove.

Important examples:

- install names are loader identity, not just filenames
- frameworks are resolved through a different search model than plain libraries
- re-export chains can involve dependency structure outside the immediate
  artifact
- symbol spelling and visibility evidence are useful, but not a complete dyld
  decision procedure

## When To Use Inventories Directly

Use `inspect_symbols(...)` directly when:

- you want to debug a native artifact before validating bindings
- you need artifact metadata without having headers available
- you want to compare two builds of the same native library
- you need archive-member or dependency-edge evidence for a linker-oriented
  workflow
