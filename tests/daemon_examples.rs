use std::path::PathBuf;

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
