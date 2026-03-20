use std::path::PathBuf;

use linc::SymbolInventory;
use linc::{validate, MatchStatus};

#[path = "../test/stress/daemon/max_pain.rs"]
mod max_pain;

#[test]
fn combined_daemon_fixture_files_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/stress/daemon");
    let header = root.join("max_pain.h");
    let source = root.join("max_pain.c");

    assert!(header.exists());
    assert!(source.exists());

    let header_text = std::fs::read_to_string(&header).unwrap();
    assert!(header_text.contains("bic_daemon_create"));
    assert!(header_text.contains("bic_plugin_descriptor"));
}

#[test]
fn combined_daemon_fixture_is_code_driven_and_consumable() {
    let environment = max_pain::max_pain_environment().unwrap();
    let config = max_pain::max_pain_header_config().unwrap();
    let result = max_pain::analyze_max_pain().unwrap();

    assert!(environment.header.ends_with("test/stress/daemon/max_pain.h"));
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "dl"));
    assert!(result.package.find_function("bic_daemon_create").is_some());
    assert!(result.package.find_function("bic_daemon_submit_packet").is_some());
    assert!(result.package.find_record("bic_daemon_packet").is_some());
    assert!(result.package.find_record("bic_daemon_config").is_some());
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct bic_daemon_packet" && layout.size > 0));
}

#[test]
fn combined_daemon_inventory_fixture_is_consumable() {
    let inventory: SymbolInventory = max_pain::daemon_core_inventory_fixture();

    assert_eq!(inventory.artifact_path, "test/stress/daemon/max_pain.o");
    assert_eq!(inventory.symbols.len(), 6);
    assert!(inventory
        .symbols
        .iter()
        .any(|symbol| symbol.name == "bic_daemon_enable_tls" && symbol.is_function));
}

#[test]
fn combined_daemon_fixture_validates_against_daemon_core_inventory() {
    let result = max_pain::analyze_max_pain().unwrap();
    let inventory = max_pain::daemon_core_inventory_fixture();
    let report = validate(&result.package, &inventory);

    let daemon_entries: Vec<_> = report
        .matches
        .iter()
        .filter(|entry| entry.name.starts_with("bic_daemon_"))
        .collect();

    assert_eq!(daemon_entries.len(), 6);
    assert!(daemon_entries
        .iter()
        .all(|entry| entry.status == MatchStatus::Matched));
    assert_eq!(
        report
            .missing()
            .iter()
            .filter(|entry| entry.name.starts_with("bic_daemon_"))
            .count(),
        0
    );
}
