#!/usr/bin/env bash
set -euo pipefail

package_name=${1:?package name is required}
crate_name=${2:?crate name is required}
expected_parc_revision=${LINC_PARC_RELEASE_REVISION:?PARC release revision is required}
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
# Keep Cargo's lexical sibling path. Resolving a development symlink here would
# make the package-time registry patch look like a different source identity.
parc_path=$(cd "$parc_path" && pwd -L)
parc_package=$(sed -n 's/^name = "\([^"]*\)"/\1/p' "$parc_path/Cargo.toml" | head -n 1)
parc_version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$parc_path/Cargo.toml" | head -n 1)
parc_revision=$(git -C "$parc_path" rev-parse HEAD)
if test "$parc_revision" != "$expected_parc_revision"; then
    echo "PARC revision mismatch: expected $expected_parc_revision, found $parc_revision" >&2
    exit 1
fi
if test -n "$(git -C "$parc_path" status --porcelain=v1 --untracked-files=all)"; then
    echo "PARC package input must be a clean worktree" >&2
    exit 1
fi

parc_archive="$target_dir/package/${parc_package}-${parc_version}.crate"
linc_archive="$target_dir/package/${package_name}-${version}.crate"
rm -f "$parc_archive" "$linc_archive"

# Package the producer first. The extracted archives are then tested together
# through a scratch workspace patch, so Cargo never falls back to a published
# PARC release while verifying the cross-crate contract.
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

for file in README.md RELEASE.md LICENSE-MIT LICENSE-APACHE; do
    test -f "$scratch/$linc_dir/$file"
done
grep -Fqx "name = \"${package_name}\"" "$scratch/$linc_dir/Cargo.toml.orig"
grep -Fqx "version = \"${version}\"" "$scratch/$linc_dir/Cargo.toml.orig"
grep -Fqx "name = \"${crate_name}\"" "$scratch/$linc_dir/Cargo.toml.orig"
grep -Fqx 'rust-version = "1.89"' "$scratch/$linc_dir/Cargo.toml.orig"
grep -Fqx 'license = "MIT OR Apache-2.0"' "$scratch/$linc_dir/Cargo.toml.orig"
grep -Fqx 'publish = false' "$scratch/$linc_dir/Cargo.toml.orig"
grep -Fq 'parc = { package = "follang-parc", version = "=0.16.0", path = "../parc" }' \
    "$scratch/$linc_dir/Cargo.toml.orig"
if awk '
    /^\[(target\..*\.)?(dependencies|dev-dependencies|build-dependencies)(\.|])/{ dependency = 1; next }
    /^\[/{ dependency = 0 }
    dependency && /^[[:space:]]*path[[:space:]]*=/{ found = 1 }
    END { exit found ? 0 : 1 }
' "$scratch/$linc_dir/Cargo.toml"; then
    echo "normalized packaged manifest contains a path dependency" >&2
    exit 1
fi
grep -Fqx 'version = "=0.16.0"' "$scratch/$linc_dir/Cargo.toml"
grep -Fqx 'package = "follang-parc"' "$scratch/$linc_dir/Cargo.toml"

for file in README.md RELEASE.md LICENSE-MIT LICENSE-APACHE; do
    test -f "$scratch/$parc_dir/$file"
done
grep -Fqx "name = \"${parc_package}\"" "$scratch/$parc_dir/Cargo.toml.orig"
grep -Fqx "version = \"${parc_version}\"" "$scratch/$parc_dir/Cargo.toml.orig"
grep -Fqx 'publish = false' "$scratch/$parc_dir/Cargo.toml.orig"

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
${crate_name} = { package = "${package_name}", version = "=${version}", default-features = false, features = ["native-inspection"] }
EOF
cat >"$scratch/consumer/src/lib.rs" <<'EOF'
use linc::contract::{
    corpus as linc_corpus, AnalysisRequest, ProbeResourceLimits, ValidatedLinkAnalysis,
};
use linc::native::{
    CertificationToolchain, InspectionLimits, NativeAnalyzer, NativeInspector, NativeResolver,
    NativeResult, ResolverConfiguration,
};
use parc::contract::{corpus as parc_corpus, decode_source_package};
use std::path::PathBuf;

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

pub fn packaged_observation_surface_is_checked(
    compiler: PathBuf,
    limits: ProbeResourceLimits,
) -> NativeResult<parc::contract::CompilerIdentity> {
    CertificationToolchain::observe(compiler, Vec::new(), limits)
        .map(|toolchain| toolchain.compiler_identity().clone())
}

pub fn packaged_certification_surface_is_checked(
    analyzer: &NativeAnalyzer,
    request: &AnalysisRequest<'_>,
    toolchain: &CertificationToolchain,
) -> NativeResult<ValidatedLinkAnalysis> {
    analyzer.certify(request, toolchain)
}

pub fn packaged_native_surface_is_checked() -> bool {
    let inspector = NativeInspector::new(InspectionLimits::default())
        .expect("packaged native inspector accepts default limits");
    let resolver = NativeResolver::new(inspector, ResolverConfiguration::default())
        .expect("packaged native resolver accepts default policy");
    let analyzer = NativeAnalyzer::new(resolver);
    analyzer.resolver().inspector().limits().max_symbols != 0
}

#[test]
fn packaged_contract_pair_roundtrips() {
    assert!(packaged_contract_pair_is_checked());
    assert!(packaged_native_surface_is_checked());
}
EOF

# Never reuse build fingerprints from an earlier extracted archive with the
# same package version. Package validation must compile the bytes just unpacked.
export CARGO_TARGET_DIR="$scratch/target"
cargo test --manifest-path "$scratch/Cargo.toml" -p "$package_name" --offline
if test "$(uname -s)" = Linux; then
    command -v cc >/dev/null 2>&1 || { echo "packaged native tests require cc"; exit 1; }
    command -v ar >/dev/null 2>&1 || { echo "packaged native tests require ar"; exit 1; }
    test -x /bin/kill || { echo "packaged native tests require /bin/kill"; exit 1; }
    "$root/tools/require-nonzero-tests.sh" packaged-native-linux \
        env LINC_TEST_CC="$(command -v cc)" LINC_TEST_AR="$(command -v ar)" \
        LINC_TEST_CLANG="$(command -v clang 2>/dev/null || true)" \
        cargo test --manifest-path "$scratch/Cargo.toml" -p "$package_name" \
            --features native-inspection --test native_evidence --offline -- \
            --nocapture --test-threads=1
fi
cargo test --manifest-path "$scratch/Cargo.toml" -p "${crate_name}-package-consumer" --offline
