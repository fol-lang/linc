#!/usr/bin/env bash
set -euo pipefail

package_name=${1:?package name is required}
crate_name=${2:?crate name is required}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)
target_dir=${CARGO_TARGET_DIR:-$root/target}
export CARGO_TARGET_DIR="$target_dir"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/${crate_name}-package.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

parc_path=$(sed -n 's/^parc = .*path = "\([^"]*\)".*/\1/p' "$root/Cargo.toml" | head -n 1)
test -n "$parc_path"
case "$parc_path" in
    /*) ;;
    *) parc_path="$root/$parc_path" ;;
esac
parc_path=$(cd "$parc_path" && pwd -P)
parc_package=$(sed -n 's/^name = "\([^"]*\)"/\1/p' "$parc_path/Cargo.toml" | head -n 1)
parc_version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$parc_path/Cargo.toml" | head -n 1)

parc_archive="$target_dir/package/${parc_package}-${parc_version}.crate"
linc_archive="$target_dir/package/${package_name}-${version}.crate"
rm -f "$parc_archive" "$linc_archive"

# Package the producer first. The extracted archives are then tested together
# through a scratch workspace patch, so Cargo never falls back to a published
# PARC release while verifying the new cross-crate contract.
cargo package --manifest-path "$parc_path/Cargo.toml" --allow-dirty --no-verify --offline
test -f "$parc_archive"
cat >"$scratch/package-config.toml" <<EOF
[patch.crates-io]
${parc_package} = { path = "$parc_path" }
EOF
cargo --config "$scratch/package-config.toml" package \
    --manifest-path "$root/Cargo.toml" --allow-dirty --no-verify --offline
test -f "$linc_archive"

tar -xzf "$parc_archive" -C "$scratch"
tar -xzf "$linc_archive" -C "$scratch"
parc_dir="${parc_package}-${parc_version}"
linc_dir="${package_name}-${version}"

mkdir -p "$scratch/consumer/src"
cat >"$scratch/Cargo.toml" <<EOF
[workspace]
members = ["$parc_dir", "$linc_dir", "consumer"]
resolver = "2"

[patch.crates-io]
${parc_package} = { path = "$parc_dir" }
${package_name} = { path = "$linc_dir" }
EOF
cat >"$scratch/consumer/Cargo.toml" <<EOF
[package]
name = "${crate_name}-package-consumer"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
parc = { package = "${parc_package}", version = "=${parc_version}", default-features = false }
${crate_name} = { package = "${package_name}", version = "=${version}", default-features = false, features = ["contracts"] }
EOF
cat >"$scratch/consumer/src/lib.rs" <<'EOF'
use linc::contract::corpus as linc_corpus;
use parc::contract::{corpus as parc_corpus, decode_source_package};

pub fn packaged_contract_pair_is_checked() -> bool {
    let source = decode_source_package(parc_corpus::COMPLETE_SOURCE_PACKAGE_JSON)
        .expect("packaged PARC corpus decodes");
    let complete = source
        .into_complete(&linc_corpus::preservation_selection())
        .expect("packaged PARC corpus proves the selected closure");
    let validated = linc_corpus::validated_preservation_link_analysis(&complete)
        .expect("packaged LINC corpus covers that closure");
    validated.package().source_fingerprint() == complete.source().fingerprint()
        && validated.package().target_fingerprint() == complete.source().target_fingerprint()
}

#[test]
fn packaged_contract_pair_roundtrips() {
    assert!(packaged_contract_pair_is_checked());
}
EOF

cargo test --manifest-path "$scratch/Cargo.toml" -p "$package_name" --offline
cargo test --manifest-path "$scratch/Cargo.toml" -p "${crate_name}-package-consumer" --offline
