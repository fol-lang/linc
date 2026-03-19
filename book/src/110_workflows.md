# End-To-End Workflows

This chapter ties the separate surfaces together into practical workflows.

## Workflow 1: Scan A Header And Save JSON

```sh
bic scan \
  --header include/demo.h \
  --include-dir include \
  > bindings.json
```

This is the baseline path for most automation.

The resulting file now contains:

- declarations
- macros
- layouts if requested
- link metadata
- diagnostics

## Workflow 2: Inspect A Native Artifact

```sh
bic inspect-symbols --file build/libdemo.so > symbols.json
```

Use this when you need artifact evidence first.

Typical reasons:

- debugging whether a build exported the symbol you expected
- checking archive member provenance
- checking shared-library dependency edges

## Workflow 3: Validate Bindings Against Artifacts

```sh
bic validate \
  --bindings-json bindings.json \
  --artifact build/libdemo.so
```

This is the first serious consistency check between header intent and native reality.

For a split native surface:

```sh
bic validate \
  --bindings-json bindings.json \
  --artifact build/libcore.so \
  --artifact build/libsupport.a
```

## Workflow 4: Extract Just The Link Surface

```sh
bic link-plan --bindings-json bindings.json > link-surface.json
```

This is a useful boundary if a downstream tool only wants:

- library names
- concrete artifact inputs
- framework inputs
- platform constraints
- ordering and link preference metadata

## Workflow 5: Preprocessed-Only Debugging

If a raw-header scan is confusing, break the problem in two:

1. produce or capture preprocessed source
2. run `scan-preprocessed`

```sh
bic scan-preprocessed --file debug.i --source-path debug.h
```

This isolates extraction behavior from compiler invocation behavior.

## Workflow 6: ABI-Sensitive Packages

For packages with important struct ABI:

```sh
bic scan \
  --header include/api.h \
  --probe-type "struct api_context" \
  --probe-type "struct api_options" \
  > bindings.json
```

Then validate against the built native artifact:

```sh
bic validate \
  --bindings-json bindings.json \
  --artifact build/libapi.so
```

This gives you:

- declaration extraction
- macro inventory
- layout evidence
- symbol-provider evidence

in one workflow.

## Workflow 7: Downstream `fol` Consumption

The intended downstream pattern is:

1. `bic scan` produces `BindingPackage`
2. `fol` reads the package JSON
3. `fol` lowers `package.items` into generated bindings
4. `fol` reads `package.link` to construct native link inputs
5. `fol` may use validation output as a gate or diagnostic surface

That division keeps `bic` focused on analysis and normalization rather than owning final build execution.

## Recommended Validation Gate

For serious native binding pipelines, a practical gate is:

- fail on `Missing`
- fail on `UnresolvedDeclaredLinkInputs`
- fail on `DuplicateProviders`
- inspect `DecorationMismatch`
- treat `WeakMatch` as policy-dependent

That is a pragmatic middle ground between "trust the headers blindly" and "pretend current validation proves full ABI compatibility".
