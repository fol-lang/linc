use std::path::PathBuf;

use bic::{DiagnosticKind, Severity};

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
fn torture_header_characterizes_current_parse_boundary() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/linus/c_interop_torture.h");
    let result = bic::HeaderConfig::new()
        .entry_header(&header)
        .include_dir("/usr/include")
        .include_dir("/usr/include/x86_64-linux-gnu")
        .no_origin_filter()
        .process()
        .unwrap();

    assert_eq!(result.package.item_count(), 0);
    assert_eq!(result.package.unsupported_count(), 0);
    assert_eq!(result.package.diagnostics.len(), 1);

    let diagnostic = &result.package.diagnostics[0];
    assert_eq!(diagnostic.kind, DiagnosticKind::ParseFailed);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(diagnostic.message.contains("c_interop_torture.h"));
    assert!(diagnostic.message.contains("line 40"));
}
