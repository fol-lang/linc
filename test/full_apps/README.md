# Full App Fixtures

This directory holds larger C programs than the micro fixtures in `test/reftests/`.

Each fixture lives in its own directory and includes a `fixture.toml` manifest plus one or
more C source or header files.

Current scope:

- synthetic single-file apps
- synthetic multi-file apps
- synthetic preprocessed snapshots
- curated external fixtures with pinned upstream metadata

Fixture modes:

- `translation_unit`: parse the entry file directly
- `driver`: run preprocessing with local include directories and then parse
- `preprocessed`: parse a pinned `.i` snapshot deterministically

Common manifest fields:

- `name`
- `mode`
- `flavor`
- `entry`
- `expected`
- `include_dirs`
- `tags`
- `source`
- `upstream_ref`
- `license`
- `notes`

External fixture provenance is tracked in `test/full_apps/EXTERNAL_SOURCES.md`.
