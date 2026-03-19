use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn temp_dir(label: &str) -> PathBuf {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bic_cli_{label}_{}_{}", std::process::id(), id));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn cli_scan_preprocessed_emits_binding_json() {
    let dir = temp_dir("preprocessed");
    let input = dir.join("api.i");
    std::fs::write(&input, "int foo(int x);\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "scan-preprocessed",
            "--file",
            input.to_str().unwrap(),
            "--source-path",
            "api.i",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["source_path"], "api.i");
    assert_eq!(json["items"].as_array().unwrap().len(), 1);
    assert!(json.get("target").is_some());
    assert!(json.get("inputs").is_some());
    assert!(json.get("link").is_some());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_scan_emits_inputs_and_link_metadata() {
    let dir = temp_dir("header");
    let header = dir.join("api.h");
    std::fs::write(&header, "int add(int a, int b);\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "scan",
            "--header",
            header.to_str().unwrap(),
            "--include-dir",
            dir.to_str().unwrap(),
            "--library-dir",
            dir.to_str().unwrap(),
            "--define",
            "API_LEVEL=1",
            "--link-lib",
            "m",
            "--no-origin-filter",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["inputs"]["entry_headers"].as_array().unwrap().len(), 1);
    assert_eq!(json["inputs"]["include_dirs"][0], dir.to_str().unwrap());
    assert_eq!(json["link"]["library_paths"][0], dir.to_str().unwrap());
    assert_eq!(json["link"]["libraries"][0]["name"], "m");
    assert_eq!(json["items"].as_array().unwrap().len(), 1);

    std::fs::remove_dir_all(&dir).ok();
}
