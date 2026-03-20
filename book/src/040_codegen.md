# Code Generation

Rust FFI code generation has been moved to **`gec`** (the GERC crate).

LINC does not own code generation. Its responsibility ends at producing
`BindingPackage` and link/binary evidence. The downstream `gec` crate
consumes that evidence to emit Rust projections.

## Migration Note

The `codegen` feature and `emit_rust_ffi` function that previously lived
in this crate have been removed. If you were using them directly, switch
to `gec` for Rust FFI generation.

## Pipeline

```text
PARC -> LINC -> BindingPackage (JSON) -> GERC -> Rust FFI
```

See the [gec documentation](https://github.com/follang/gec) for code
generation details.
