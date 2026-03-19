# Stable Usage Patterns

This chapter describes the usage patterns that are most likely to remain stable for downstream
library consumers.

The goal is to steer integrations toward durable habits rather than clever short-term shortcuts.

## Pattern 1: Prefer Root-Level Entry Points

The recommended entry points are:

- `HeaderConfig`
- `PreprocessedInput`
- `to_json` / `from_json`
- `probe_type_layouts`
- `inspect_symbols`
- `validate` / `validate_many`

This is safer than building a workflow around deep module imports unless you are deliberately
writing advanced integration code.

## Pattern 2: Treat `BindingPackage` As The Primary Product

For most consumers, the most stable intermediate artifact is the package itself.

A typical flow is:

1. scan headers or parse preprocessed input
2. inspect `package.diagnostics`
3. optionally attach or compare native evidence
4. serialize the package only when crossing a tool boundary

This is more stable than reconstructing intent from raw AST-level details.

## Pattern 3: Treat Diagnostics As Contractual Data

Do not treat diagnostics as incidental log text.

The stable pattern is:

- read `package.diagnostics`
- classify which diagnostic kinds are blocking for your downstream generator
- make the decision explicit in your consumer

That is more durable than guessing from whether an operation happened to return `Ok(...)`.

## Pattern 4: Gate Compatibility On `schema_version`

If your tool stores or consumes package JSON:

- gate on `schema_version`
- treat `bic_version` as provenance, not wire compatibility
- keep fixture coverage for the payload shapes you rely on

This is safer than keying behavior off the producing crate version string.

## Pattern 5: Preserve Native Metadata Instead Of Re-Deriving It

When `package.link`, `package.layouts`, or symbol inventories are available, prefer using them.

Do not assume your downstream generator should reconstruct:

- native search paths
- declared libraries
- declared frameworks
- layout evidence
- platform applicability

That information is part of the intended machine contract.

## Pattern 6: Keep Validation Separate From Transport Failure

The stable consumer pattern is:

- execution failure -> handled as `Err(...)`
- successful validation with mismatches -> handled as structured evidence

Do not collapse these into the same error channel.

## Pattern 7: Isolate Lower-Level Module Usage

If you need module-level APIs such as `extract`, `symbols`, or `raw_headers`, isolate that code in
one small adapter layer inside your consumer.

That way:

- most of your code depends only on root-level contracts
- any future API cleanup is localized
- the integration remains easier to review and stabilize

## Anti-Patterns

These patterns are likely to age badly:

- matching on exact free-form error strings
- treating pretty JSON formatting as a semantic contract
- assuming every successful scan is generation-ready
- depending broadly on support-oriented modules for normal consumer flows
- inferring link intent only from declarations when `package.link` already exists
