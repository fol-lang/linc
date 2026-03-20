# Intake Layer

The intake layer is LINC's frontend-neutral input contract. It defines what
LINC needs from a frontend without coupling to any specific parser AST.

## SourcePackage

The primary intake type is `SourcePackage`. A frontend (like `parc`) produces
this after scanning and extracting source-level information.

```rust
use linc::{SourcePackage, SourceDeclaration, SourceFunction, SourceType};
use linc::from_source_package;

let mut src = SourcePackage::default();
src.source_path = Some("mylib.h".into());
src.declarations.push(SourceDeclaration::Function(SourceFunction {
    name: "init".into(),
    parameters: vec![],
    return_type: SourceType::Int,
    variadic: false,
    source_offset: None,
}));

let package = from_source_package(&src);
```

## Declaration Types

The intake layer supports these declaration kinds:

- `SourceFunction` — function declarations
- `SourceRecord` — struct/union declarations (opaque when `fields` is `None`)
- `SourceEnum` — enum declarations with variants
- `SourceTypeAlias` — typedef/alias declarations
- `SourceVariable` — external variable declarations

## Type Model

`SourceType` is a simplified, language-neutral type representation:

- Primitive types: `Void`, `Bool`, `Char`, `Int`, `UInt`, `Long`, etc.
- Pointers: `Pointer(inner)`, `ConstPointer(inner)`
- Arrays: `Array(element, size)`
- Function pointers: `FunctionPointer { return_type, parameters, variadic }`
- References: `TypedefRef(name)`, `RecordRef(name)`, `EnumRef(name)`
- Qualifiers: `Const(inner)`, `Volatile(inner)`

## Adapters

The `intake::adapters` module provides bidirectional conversion:

- `from_binding_package` — convert existing `BindingPackage` to `SourcePackage`
- `to_binding_package` — convert `SourcePackage` to `BindingPackage` (used by `from_source_package`)

## Design Principles

1. LINC core logic should say "analyze this normalized source surface", not "parse this"
2. Adapter code is separate from core analysis logic
3. `parc` can be a dev dependency in tests, but not in core architecture
4. Another frontend should be able to replace `parc` without rewriting LINC
