mod common;

use linc::raw_headers::HeaderConfig;
use linc::{DiagnosticKind, LincError};

fn temp_root(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("linc_failure_matrix_{tag}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn failure_matrix_probe_invalid_config_is_typed_error() {
    let err = common::process(&HeaderConfig::new().entry_header("")).expect_err("empty entry path should fail validation");
    match err {
        LincError::InvalidConfig { .. } => {}
        other => panic!("expected invalid config error, got {other:?}"),
    }
}

#[test]
fn failure_matrix_probe_unavailable_and_failed_are_distinct() {
    let dir = temp_root("probe");
    let header = dir.join("probe.h");
    std::fs::write(
        &header,
        "typedef struct opaque_widget opaque_widget;\n\
         extern int opaque_use(opaque_widget *widget);\n\
         extern int concrete_use(int value);\n",
    )
    .unwrap();

    let unavailable = common::process(&HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("struct opaque_widget"))
        .unwrap();
    assert_eq!(unavailable.package.probe_unavailable_count(), 1);
    assert_eq!(unavailable.package.probe_failure_count(), 0);
    assert!(unavailable
        .package
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::ProbeUnavailable));

    let failed = common::process(&HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("struct invalid["))
        .unwrap();
    assert_eq!(failed.package.probe_unavailable_count(), 0);
    assert_eq!(failed.package.probe_failure_count(), 1);
    assert!(failed
        .package
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::ProbeFailed));

    std::fs::remove_dir_all(&dir).ok();
}
