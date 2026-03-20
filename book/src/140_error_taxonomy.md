# Error Taxonomy

This chapter defines the intended typed error taxonomy that later implementation slices should converge on.

It is a design slice first.
It does not yet mean every operational API has been migrated.

## Goal

`LincError` should become the crate-wide typed failure surface for operational failures.

The design target is to separate these concerns cleanly:

- configuration and scan execution failures
- preprocessing and parse failures
- ABI probe failures
- artifact inspection failures
- serialization and schema failures

Validation findings are deliberately different.
They should remain structured report output, not thrown as operational errors.

## Current `LincError` Coverage

The current enum already contains variants for:

- missing headers
- preprocessor failure
- parse failure
- I/O failure
- serialization failure
- symbol-read failure
- unsupported artifact format
- schema-version mismatch

That is a decent starting point, but it is not yet the final operational taxonomy.

## Intended Category Boundaries

## 1. Scan And Configuration Failures

These are failures such as:

- missing entry headers
- invalid or contradictory scan configuration
- scan setup that cannot be normalized safely

The important design rule is:

- configuration failure should be distinguishable from compiler failure

## 2. Preprocessing And Parse Failures

These cover:

- compiler/preprocessor invocation failures
- parse failures after preprocessing

The important design rule is:

- consumers should be able to distinguish "the toolchain invocation failed" from "the source did not parse"

## 3. Probe Failures

These cover ABI/layout probe execution problems such as:

- compiler invocation failure
- probe program compilation failure
- probe output parse failure

The important design rule is:

- probe failures should not be collapsed into generic scan or I/O text

## 4. Artifact Inspection Failures

These cover:

- unreadable native artifacts
- unsupported native formats
- malformed or unparseable artifact contents

The important design rule is:

- path/context should be preserved in the typed error

## 5. Serialization And Schema Failures

These cover:

- JSON serialization/deserialization failures
- schema-version mismatch

This is currently the most mature typed-error area in the crate.

## Validation Is Not An Error Channel

This point matters enough to say twice:

- validation mismatches should not be encoded as `LincError`
- validation should keep returning `ValidationReport`

That keeps operational failure separate from analytical result data.

## Migration Direction

The intended migration order is:

1. define the taxonomy
2. migrate raw-header scanning
3. migrate probing
4. migrate symbol inspection
5. add stable tests for error categories and messages

That sequence avoids changing several operational surfaces without a clear target model.
