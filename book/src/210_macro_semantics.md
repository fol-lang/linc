# Macro Semantics

This chapter defines the intended stable semantics for captured macros.

Macro inventory alone is not enough for a downstream binding generator.
The useful contract is which macro information is safe to consume directly, which is advisory, and
which should be treated as unsupported analysis evidence.

## Current Macro Contract

Each captured macro currently carries:

- `name`
- `body`
- `function_like`
- `form`
- `kind`
- `category`
- optional parsed `value` for bindable integer/string constants

These fields should be read with different confidence levels.

## Intended Semantics By Category

### `BindableConstant`

These are the macros a downstream generator may most reasonably attempt to lower into a language
constant surface.

Current expectations:

- object-like integer and string macros are the primary intended members of this category
- when parsing succeeds, `value` gives the explicit lowered integer/string form
- expression-like macros may still need consumer caution even when categorized as bindable
- downstream code should still inspect the raw `body` if exact lowering matters

### `ConfigurationFlag`

These macros are not primarily binding targets.
They are part of the effective compile-time environment.

Downstream tools should use them to answer questions like:

- which feature/profile was active during extraction
- whether the observed ABI surface depends on compile-time configuration

### `AbiAffecting`

These macros matter because they may change type/layout/API shape rather than because they are good
binding constants themselves.

They should be treated as audit signals.

### `Unsupported`

These macros are intentionally preserved as evidence, not as directly bindable output.

Downstream generators should not assume they can lower them safely without extra logic.

## Function-Like vs Object-Like

The first stable rule is:

- object-like macros are the main candidates for binding constants
- function-like macros should be treated conservatively unless future slices add stronger lowering
  rules

That means a macro being captured does not automatically make it a good codegen target.

## Consumer Guidance

For now, downstream consumers should:

1. treat macro capture as structured evidence, not as guaranteed language-lowering input
2. prefer `BindableConstant` object-like macros first
3. treat `ConfigurationFlag` and `AbiAffecting` as environment/provenance data
4. treat `Unsupported` macros as preserved but not directly bindable

## Future Direction

Later macro slices are expected to add:

- explicit constant-value lowering for integer/string macros
- clearer handling for unsupported function-like macros
- provenance and source-location data
- richer effective macro-environment reporting

This chapter sets the semantic policy before those richer representations arrive.
