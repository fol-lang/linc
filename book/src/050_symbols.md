# Symbol Inventories

When the `symbols` feature is enabled, LINC can inspect native artifacts and produce a `SymbolInventory`.

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
- optional `alias_of`

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

`alias_of` is preserved when LINC can see more than one exported symbol name resolving to the
same section/address identity. That is intentionally conservative: LINC only records alias
relationships when the artifact evidence is strong enough.

## ELF Symbol Versions

On ELF artifacts, `SymbolEntry.version` preserves symbol-version evidence when the object reader
can see it, for example `GLIBC_2.2.5` or a library-specific export namespace.

Downstream consumers should read that evidence conservatively:

- version presence is useful provider metadata
- version absence is not proof that the symbol is unversioned everywhere
- version equality helps distinguish exports that share a base symbol name
- version differences should be treated as a reason to avoid collapsing providers too aggressively

Today LINC does not implement a full ELF linker/version-script resolver.
The intended policy is narrower:

- use normalized name as the primary declaration/provider match key
- preserve version strings as attached evidence
- surface them to downstream policy code when provider selection needs to stay conservative

That means version evidence is best read as "stronger provider identity context", not as a final
dynamic-loader decision.

## Archive Member Provenance

For static libraries, LINC preserves the member path/name that provided each symbol when available.

That lets downstream validation report a provider more precisely than just:

```text
libfoo.a
```

It can instead report:

```text
libfoo.a:bar.o
```

## Shared-Library Dependency Edges

On ELF shared libraries and executables, LINC now captures `DT_NEEDED` dependencies into `dependency_edges`.

This is not a full dynamic-loader model.
It is still useful because it exposes artifact-declared native dependencies in the inventory itself.

Example values might look like:

- `libm.so.6`
- `libc.so.6`
- `libz.so.1`

When LINC sees imported symbols inside a shared library or executable, it also preserves
symbol-local `reexported_via` evidence using those dependency edges. That is still an inference
layer, not proof of a platform loader decision, but it is much stronger than a plain artifact-wide
"this file has dependencies" signal.

## Platform Behavior Notes

Mach-O commonly prefixes external symbols with `_`.
LINC normalizes those names so C declarations and native symbols compare more naturally.

That normalization is intentionally paired with `raw_name` preservation so no spelling evidence is lost.

Mach-O support should still be read conservatively:

- imported symbols are useful dependency evidence, not proof of final loader behavior
- re-export inferences are intentionally narrower than a full dyld model
- framework and install-name semantics remain downstream policy concerns
- normalized names are for matching, while `raw_name` stays the authoritative original spelling

In other words, Mach-O inventories are strong enough for serious validation and planning workflows,
but they are still evidence surfaces rather than a full Apple linker/loader simulation.

## Mach-O Limits And Conservative Provider Policy

Downstream consumers should treat Mach-O provider evidence more conservatively than straightforward
ELF export evidence.

That is not because the current inventories are weak.
It is because Mach-O linking and loading semantics often depend on more context than a plain symbol
table can prove by itself.

Important examples:

- install names are loader identity, not just filenames
- frameworks are resolved through a different search model than plain libraries
- re-export chains can involve dependency structure outside the immediate artifact
- symbol spelling and visibility evidence are useful, but not a complete dyld decision procedure

For that reason, the recommended LINC consumer policy is:

- treat direct exported-symbol evidence as the strongest provider signal
- treat imported symbols as dependency evidence, not as providers
- treat `reexported_via` as conservative evidence of a possible forwarding path
- avoid collapsing multiple visible candidates into one "obvious" provider without additional policy
- keep artifact path, raw symbol spelling, and dependency edges available to downstream reporting

In practice this means:

- `Resolved` should mean "the current evidence set has one conservative provider candidate"
- `Ambiguous` should remain ambiguous when multiple visible candidates plausibly satisfy the same
  declaration
- lack of a Mach-O-specific signal should not be reinterpreted as proof that a symbol is
  unavailable on Apple platforms

LINC is intentionally modeling inventory and validation evidence here.
It is not attempting to be a full replacement for `ld64`, `dyld`, framework lookup rules, or
platform-specific packaging conventions.

## When To Use Inventories Directly

Use `inspect_symbols(...)` directly when:

- you want to debug a native artifact before validating bindings
- you need artifact metadata without having headers available
- you want to compare two builds of the same native library
- you need archive-member or dependency-edge evidence for a linker-oriented workflow
