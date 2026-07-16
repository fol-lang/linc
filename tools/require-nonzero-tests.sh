#!/usr/bin/env bash
set -euo pipefail

label=${1:?test-lane label is required}
shift
output=$(mktemp "${TMPDIR:-/tmp}/linc-${label}.XXXXXX")
trap 'rm -f "$output"' EXIT

if ! "$@" >"$output" 2>&1; then
    cat "$output"
    exit 1
fi
cat "$output"

if ! grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$output"; then
    echo "${label} lane passed zero tests" >&2
    exit 1
fi
