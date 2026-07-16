mod common;
use std::path::PathBuf;

use linc::DiagnosticKind;

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn torture_header_scans_through_public_header_config() {
    eprintln!("RUN: host C compiler torture-header scan evidence");
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/linus/c_interop_torture.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .include_dir("/usr/include")
            .include_dir("/usr/include/x86_64-linux-gnu")
            .no_origin_filter(),
    )
    .unwrap();

    assert!(result.report.preprocessed_source.contains("torture_open"));
    assert!(result
        .report
        .preprocessed_source
        .contains("struct torture_config"));
}

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn torture_header_recovers_packed_typedef_declarations() {
    eprintln!("RUN: host C compiler packed-typedef recovery evidence");
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/linus/c_interop_torture.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .include_dir("/usr/include")
            .include_dir("/usr/include/x86_64-linux-gnu")
            .no_origin_filter(),
    )
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
#[ignore = "system prerequisite: host C compiler"]
fn torture_header_still_supports_layout_probes() {
    eprintln!("RUN: host C compiler torture-header layout evidence");
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/linus/c_interop_torture.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .include_dir("/usr/include")
            .include_dir("/usr/include/x86_64-linux-gnu")
            .no_origin_filter()
            .probe_type_layout("struct torture_config")
            .probe_type_layout("struct torture_packet")
            .probe_type_layout("struct torture_buffer"),
    )
    .unwrap();

    assert!(!result.package.diagnostics.is_empty());
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
#[ignore = "system prerequisite: host C compiler"]
fn aligned_torture_header_recovers_aligned_typedef_declarations() {
    eprintln!("RUN: host C compiler aligned-typedef recovery evidence");
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/linus/c_interop_torture_aligned.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .include_dir("/usr/include")
            .include_dir("/usr/include/x86_64-linux-gnu")
            .no_origin_filter()
            .probe_type_layout("struct torture_aligned_packet"),
    )
    .unwrap();

    assert!(result
        .report
        .preprocessed_source
        .contains("torture_aligned_size"));
    assert!(result
        .package
        .find_record("torture_aligned_packet")
        .is_some());
    assert!(result
        .package
        .find_function("torture_aligned_size")
        .is_some());
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
