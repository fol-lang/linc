# Symbol Inventories

When the `symbols` feature is enabled, `bic` can inspect native artifacts and produce a `SymbolInventory`.

This is the artifact-side counterpart to `BindingPackage`.

## Why Symbol Inventories Matter

Header extraction tells you what the C surface claims exists.
Artifact inspection tells you what a native file actually exports or imports.

You need both when you want to answer questions such as:

- does this library really provide the declarations I scanned?
- which artifact satisfies a symbol?
- is the symbol hidden, weak, or duplicated?
- what shared-library dependencies does this artifact declare?

## Entry Point

```rust
use bic::inspect_symbols;

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

`capabilities` currently capture whether an artifact:

- exports symbols
- imports symbols

That distinction matters for differentiating linkable providers from dependency-only inputs.

## Symbol Entries

Each `SymbolEntry` carries:

- normalized `name`
- optional `raw_name`
- `direction` (`Exported` or `Imported`)
- `visibility`
- whether it is a function or variable-like symbol
- `binding`
- optional `size`
- optional `section`
- optional `archive_member`
- optional `reexported_via`

### Normalized vs Raw Name

The normalized name is used for matching declarations.
The raw name preserves the original artifact spelling.

This is important because native artifacts may use:

- leading underscore decoration
- other platform-specific symbol spellings
- archive member-local provenance

`direction` is also important now: only exported symbols are candidate providers during
validation. Imported symbols are still preserved because they matter for shared-library and
link-planning analysis.

## Archive Member Provenance

For static libraries, `bic` preserves the member path/name that provided each symbol when available.

That lets downstream validation report a provider more precisely than just:

```text
libfoo.a
```

It can instead report:

```text
libfoo.a:bar.o
```

## Shared-Library Dependency Edges

On ELF shared libraries and executables, `bic` now captures `DT_NEEDED` dependencies into `dependency_edges`.

This is not a full dynamic-loader model.
It is still useful because it exposes artifact-declared native dependencies in the inventory itself.

Example values might look like:

- `libm.so.6`
- `libc.so.6`
- `libz.so.1`

When `bic` sees imported symbols inside a shared library or executable, it also preserves
symbol-local `reexported_via` evidence using those dependency edges. That is still an inference
layer, not proof of a platform loader decision, but it is much stronger than a plain artifact-wide
"this file has dependencies" signal.

## Platform Behavior Notes

Mach-O commonly prefixes external symbols with `_`.
`bic` normalizes those names so C declarations and native symbols compare more naturally.

That normalization is intentionally paired with `raw_name` preservation so no spelling evidence is lost.

## When To Use Inventories Directly

Use `inspect_symbols(...)` directly when:

- you want to debug a native artifact before validating bindings
- you need artifact metadata without having headers available
- you want to compare two builds of the same native library
- you need archive-member or dependency-edge evidence for a linker-oriented workflow
