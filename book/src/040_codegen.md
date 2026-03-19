# Code Generation

When the `codegen` feature is enabled, `bic` can emit Rust FFI from a `BindingPackage`.

That emitter is intentionally straightforward.
It is useful for bootstrapping interop, testing extraction quality, and providing a baseline Rust-facing output.

## Entry Point

```rust
use bic::emit_rust_ffi;

let rust_code = emit_rust_ffi(&package);
```

The input is the extracted IR, not raw C source.
That means generation quality depends directly on extraction quality.

## What The Rust Emitter Handles Well

The emitter is designed for classic C FFI surface shapes:

- `extern "C"` function declarations
- global variables
- `#[repr(C)]` structs
- `#[repr(C)]` unions
- typedefs
- enum constants
- function pointer typedefs and fields
- opaque forward-declared types

## Typical Output Shape

Generated output generally includes:

- `pub type` aliases
- `#[repr(C)]` record definitions
- `extern "C"` blocks
- `pub static` declarations for globals
- enum constants or equivalent lowered forms

## Example

```rust
use bic::{extract_from_source, emit_rust_ffi};

let pkg = extract_from_source(
    r#"
    typedef unsigned int flags_t;
    struct point { int x; int y; };
    int add(int a, int b);
    extern int global_counter;
    "#,
).unwrap();

let out = emit_rust_ffi(&pkg);
println!("{out}");
```

## Opaque Records

Forward declarations such as:

```c
struct FILE;
```

are emitted as opaque Rust-compatible types.

This is one of the most important behaviors for real interop because many C APIs intentionally hide implementation details.

## Function Pointers

`bic` preserves function-pointer types and the Rust emitter lowers them into FFI-compatible signatures.

This is particularly useful for:

- callback-heavy APIs
- vtable-like configuration structs
- typedef-based function pointer patterns

## Limits To Keep In Mind

The Rust generator is not a full safe wrapper generator.
It emits FFI declarations, not ergonomic Rust APIs.

In particular, it does not solve:

- ownership
- lifetime modeling
- error translation
- safe wrapper construction
- target-conditional public API partitioning

It also inherits the current IR limitations.
For example, if a source construct was reduced to an unsupported item or a warning-level approximation, the emitter cannot restore missing semantics later.

## Recommended Use

Treat the Rust emitter as one of three things:

1. a smoke test for extraction quality
2. a baseline raw FFI layer
3. a development aid while building richer downstream generators

If you are integrating with `fol`, the JSON package and link/validation data usually matter more than the emitted Rust itself.
