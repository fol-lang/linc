use std::path::PathBuf;

use linc::{inspect_symbols, LincError};

#[test]
fn typed_error_matrix_missing_artifact_is_symbol_read() {
    let err = inspect_symbols("/definitely/not/a/real/artifact.o").unwrap_err();
    assert!(matches!(err, LincError::SymbolRead { .. }));
}

#[test]
fn typed_error_matrix_plain_text_artifact_is_unsupported_format() {
    let path = temp_path("plain_text");
    std::fs::write(&path, "not an object file at all\n").unwrap();

    let err = inspect_symbols(&path).unwrap_err();
    assert!(matches!(err, LincError::UnsupportedFormat { .. }));

    std::fs::remove_file(path).ok();
}

#[test]
fn typed_error_matrix_invalid_json_maps_to_serialization() {
    let err: LincError = serde_json::from_str::<linc::ir::BindingPackage>("not json")
        .unwrap_err()
        .into();
    assert!(matches!(err, LincError::Serialization(_)));
}

#[test]
fn typed_error_matrix_structured_json_shape_failures_map_to_serialization() {
    let json = r#"{ "schema_version": "not-a-number", "source_path": null, "items": [] }"#;
    let err = serde_json::from_str::<linc::ir::BindingPackage>(json).unwrap_err();
    let linc_err: LincError = err.into();
    assert!(matches!(linc_err, LincError::Serialization(_)));
}

fn temp_path(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "linc_typed_error_matrix_{tag}_{}_{}",
        std::process::id(),
        nanos
    ))
}
