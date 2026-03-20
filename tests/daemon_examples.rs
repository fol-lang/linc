use std::path::PathBuf;

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
