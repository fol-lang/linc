use std::path::PathBuf;

use bic::DiagnosticKind;

#[test]
fn torture_header_scans_through_public_header_config() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/linus/c_interop_torture.h");
    let result = bic::HeaderConfig::new()
        .entry_header(&header)
        .include_dir("/usr/include")
        .include_dir("/usr/include/x86_64-linux-gnu")
        .no_origin_filter()
        .process()
        .unwrap();

    assert!(result.report.preprocessed_source.contains("torture_open"));
    assert!(result.report.preprocessed_source.contains("struct torture_config"));
}

#[test]
fn torture_header_recovers_packed_typedef_declarations() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/linus/c_interop_torture.h");
    let result = bic::HeaderConfig::new()
        .entry_header(&header)
        .include_dir("/usr/include")
        .include_dir("/usr/include/x86_64-linux-gnu")
        .no_origin_filter()
        .process()
        .unwrap();

    assert!(result.package.item_count() >= 7);
    assert!(result.package.find_record("torture_config").is_some());
    assert!(result.package.find_record("torture_packet").is_some());
    assert!(result.package.find_record("torture_buffer").is_some());
    assert!(result.package.find_function("torture_open").is_some());
    assert!(result.package.find_function("torture_send").is_some());
    assert!(!result
        .package
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::ParseFailed));
    assert!(result
        .package
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::DeclarationPartial));
}

#[test]
fn torture_header_still_supports_layout_probes() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/linus/c_interop_torture.h");
    let result = bic::HeaderConfig::new()
        .entry_header(&header)
        .include_dir("/usr/include")
        .include_dir("/usr/include/x86_64-linux-gnu")
        .no_origin_filter()
        .probe_type_layout("struct torture_config")
        .probe_type_layout("struct torture_packet")
        .probe_type_layout("struct torture_buffer")
        .process()
        .unwrap();

    assert!(result.package.diagnostics.len() >= 1);
    assert!(result
        .package
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::DeclarationPartial));
    assert!(result
        .package
        .macros
        .iter()
        .any(|macro_binding| macro_binding.name == "TORTURE_API_LEVEL"));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct torture_config" && layout.size > 0));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct torture_packet" && layout.size > 0));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct torture_buffer" && layout.size > 0));
}

#[test]
fn aligned_torture_header_characterizes_current_boundary() {
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/linus/c_interop_torture_aligned.h");
    let result = bic::HeaderConfig::new()
        .entry_header(&header)
        .include_dir("/usr/include")
        .include_dir("/usr/include/x86_64-linux-gnu")
        .no_origin_filter()
        .probe_type_layout("struct torture_aligned_packet")
        .process()
        .unwrap();

    assert!(result
        .report
        .preprocessed_source
        .contains("torture_aligned_size"));
    assert!(result
        .package
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::ParseFailed));
    assert!(result
        .package
        .macros
        .iter()
        .any(|macro_binding| macro_binding.name == "TORTURE_ALIGNED_LEVEL"));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct torture_aligned_packet" && layout.size > 0));
}
