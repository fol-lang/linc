# Reproducibility

This chapter defines the intended reproducibility posture for the test and fixture environment.

The goal is not "bit-for-bit identical on every machine" for every external toolchain interaction.
The goal is that regressions are explainable, fixtures are stable, and platform-dependent tests are
clearly classified.

## Reproducibility Requirements

The current practical requirements are:

- checked-in JSON contract fixtures must be deterministic
- library-only unit tests should be deterministic without requiring internet access
- platform/toolchain-dependent tests should be explicit about their assumptions
- ignored or environment-dependent tests should not hide core contract regressions

## Fixture Rules

For stable fixtures:

- prefer checked-in headers, JSON payloads, and small native test artifacts where practical
- avoid depending on ambient machine state when a fixture can encode the case directly
- keep fixture names tied to the contract or bug they protect

## Toolchain-Dependent Tests

Some tests naturally depend on the local compiler, linker, or system libraries.

For those tests:

- the dependency should be visible in the test name or test setup
- success criteria should avoid brittle formatting assumptions
- failures should be diagnosable from the test itself, not from tribal knowledge

## Ignored Tests

Ignored tests are acceptable when they cover:

- optional system-library flows
- environment-sensitive artifact checks
- expensive end-to-end paths that are not required on every local run

They are not a substitute for protecting the main public contract.

As environment assumptions become realistic for normal development, higher-value tests should move
out of ignored status. That now includes several native-path checks such as archive-member
provenance, dependency-edge capture, macro capture, layout attachment, and end-to-end validation.

## Downstream Implication

For `fol` and other consumers, the most reliable fixtures to depend on are:

- versioned JSON contract fixtures
- small deterministic input headers
- explicit acceptance fixtures that live in source control

That gives a stronger integration boundary than relying only on whatever happens to be installed on
the developer machine.
