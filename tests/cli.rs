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
    std::fs::write(
        &header,
        "#define API_LEVEL 1\n#define API_NAME \"demo\"\ntypedef unsigned int value_t;\nint add(int a, int b);\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "scan",
            "--header",
            header.to_str().unwrap(),
            "--include-dir",
            dir.to_str().unwrap(),
            "--framework-dir",
            "/System/Library/Frameworks",
            "--library-dir",
            dir.to_str().unwrap(),
            "--define",
            "API_LEVEL=1",
            "--link-lib",
            "m",
            "--link-framework",
            "Security",
            "--link-object",
            "build/plugin.o",
            "--link-static-artifact",
            "lib/libcrypto.a",
            "--prefer-static",
            "--target-constraint",
            "linux",
            "--target-constraint",
            "x86_64",
            "--probe-type",
            "value_t",
            "--no-origin-filter",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["inputs"]["entry_headers"].as_array().unwrap().len(), 1);
    assert_eq!(json["inputs"]["include_dirs"][0], dir.to_str().unwrap());
    assert_eq!(json["link"]["framework_paths"][0], "/System/Library/Frameworks");
    assert_eq!(json["link"]["library_paths"][0], dir.to_str().unwrap());
    assert_eq!(json["link"]["preferred_mode"], "PreferStatic");
    assert_eq!(json["link"]["native_surface_kind"], "Mixed");
    assert_eq!(json["link"]["platform_constraints"][0], "linux");
    assert_eq!(json["link"]["platform_constraints"][1], "x86_64");
    assert_eq!(json["link"]["frameworks"][0]["name"], "Security");
    assert_eq!(json["link"]["frameworks"][0]["source"], "Declared");
    assert_eq!(json["link"]["libraries"][0]["name"], "m");
    assert_eq!(json["link"]["libraries"][0]["source"], "Declared");
    assert_eq!(json["link"]["artifacts"][0]["path"], "build/plugin.o");
    assert_eq!(json["link"]["artifacts"][0]["kind"], "Object");
    assert_eq!(json["link"]["artifacts"][0]["source"], "Declared");
    assert_eq!(json["link"]["artifacts"][1]["path"], "lib/libcrypto.a");
    assert_eq!(json["link"]["artifacts"][1]["kind"], "StaticLibrary");
    assert_eq!(json["link"]["ordered_inputs"].as_array().unwrap().len(), 4);
    assert_eq!(json["link"]["ordered_inputs"][0]["Library"]["name"], "m");
    assert_eq!(json["link"]["ordered_inputs"][1]["Framework"]["name"], "Security");
    assert_eq!(json["link"]["ordered_inputs"][2]["Artifact"]["path"], "build/plugin.o");
    assert_eq!(json["link"]["ordered_inputs"][3]["Artifact"]["path"], "lib/libcrypto.a");
    assert!(json["macros"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| {
            m["name"] == "API_LEVEL"
                && m["kind"] == "Integer"
                && m["category"] == "ConfigurationFlag"
        }));
    assert!(json["macros"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| {
            m["name"] == "API_NAME"
                && m["kind"] == "String"
                && m["category"] == "BindableConstant"
        }));
    assert!(json["layouts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|layout| layout["name"] == "value_t"));
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item["TypeAlias"]["name"] == "value_t"));
    assert!(items.iter().any(|item| item["Function"]["name"] == "add"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_inspect_symbols_emits_symbol_inventory_json() {
    let dir = temp_dir("symbols");
    let c_path = dir.join("lib.c");
    let o_path = dir.join("lib.o");
    std::fs::write(&c_path, "int foo(void) { return 7; }\n").unwrap();

    let status = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o_path)
        .arg(&c_path)
        .status()
        .unwrap();
    assert!(status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args(["inspect-symbols", "--file", o_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["platform"], "Elf");
    assert_eq!(json["kind"], "Object");
    assert_eq!(json["capabilities"]["exports_symbols"], true);
    assert_eq!(json["capabilities"]["imports_symbols"], false);
    let symbols = json["symbols"].as_array().unwrap();
    assert!(symbols
        .iter()
        .any(|sym| sym["name"] == "foo" && sym["raw_name"] == "foo"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_inspect_symbols_emits_archive_member_provenance() {
    let dir = temp_dir("archive_symbols");
    let c_path = dir.join("lib.c");
    let o_path = dir.join("lib.o");
    let a_path = dir.join("libtest.a");
    std::fs::write(&c_path, "int foo(void) { return 7; }\n").unwrap();

    let cc_status = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o_path)
        .arg(&c_path)
        .status()
        .unwrap();
    assert!(cc_status.success());

    let ar_status = Command::new("ar")
        .args(["rcs"])
        .arg(&a_path)
        .arg(&o_path)
        .status()
        .unwrap();
    assert!(ar_status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args(["inspect-symbols", "--file", a_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["platform"], "Elf");
    assert_eq!(json["kind"], "StaticLibrary");
    assert_eq!(json["capabilities"]["exports_symbols"], true);
    let symbols = json["symbols"].as_array().unwrap();
    assert!(symbols
        .iter()
        .any(|sym| {
            sym["name"] == "foo"
                && sym["raw_name"] == "foo"
                && sym["archive_member"] == "lib.o"
        }));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_validate_emits_validation_report_json() {
    let dir = temp_dir("validate");
    let bindings = dir.join("bindings.json");
    let c_path = dir.join("lib.c");
    let o_path = dir.join("lib.o");
    let c2_path = dir.join("lib2.c");
    let o2_path = dir.join("lib2.o");

    std::fs::write(
        &bindings,
        serde_json::json!({
            "schema_version": 1,
            "bic_version": "0.1.0",
            "target": {},
            "inputs": {},
            "link": {},
            "source_path": "api.h",
            "items": [
                {
                    "Function": {
                        "name": "foo",
                        "calling_convention": "C",
                        "parameters": [],
                        "return_type": "Int",
                        "variadic": false,
                        "source_offset": null
                    }
                },
                {
                    "Function": {
                        "name": "missing",
                        "calling_convention": "C",
                        "parameters": [],
                        "return_type": "Int",
                        "variadic": false,
                        "source_offset": null
                    }
                }
            ],
            "diagnostics": []
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(&c_path, "int foo(void) { return 7; }\n").unwrap();
    std::fs::write(&c2_path, "int other(void) { return 11; }\n").unwrap();

    let status = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o_path)
        .arg(&c_path)
        .status()
        .unwrap();
    assert!(status.success());
    let status2 = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o2_path)
        .arg(&c2_path)
        .status()
        .unwrap();
    assert!(status2.success());

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "validate",
            "--bindings-json",
            bindings.to_str().unwrap(),
            "--artifact",
            o_path.to_str().unwrap(),
            "--artifact",
            o2_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let matches = json["matches"].as_array().unwrap();
    assert!(matches.iter().any(|entry| {
        entry["name"] == "foo"
            && entry["status"] == "Matched"
            && entry["provider_artifacts"].as_array().unwrap() == &vec![serde_json::Value::String(o_path.to_str().unwrap().to_string())]
    }));
    assert!(matches.iter().any(|entry| {
        entry["name"] == "missing"
            && entry["status"] == "Missing"
            && entry["provider_artifacts"].as_array().unwrap().is_empty()
    }));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_validate_reports_duplicate_providers() {
    let dir = temp_dir("validate_dupes");
    let bindings = dir.join("bindings.json");
    let c1_path = dir.join("lib1.c");
    let o1_path = dir.join("lib1.o");
    let c2_path = dir.join("lib2.c");
    let o2_path = dir.join("lib2.o");

    std::fs::write(
        &bindings,
        serde_json::json!({
            "schema_version": 1,
            "bic_version": "0.1.0",
            "target": {},
            "inputs": {},
            "link": {},
            "source_path": "api.h",
            "items": [
                {
                    "Function": {
                        "name": "foo",
                        "calling_convention": "C",
                        "parameters": [],
                        "return_type": "Int",
                        "variadic": false,
                        "source_offset": null
                    }
                }
            ],
            "diagnostics": []
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(&c1_path, "int foo(void) { return 7; }\n").unwrap();
    std::fs::write(&c2_path, "int foo(void) { return 11; }\n").unwrap();

    let status1 = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o1_path)
        .arg(&c1_path)
        .status()
        .unwrap();
    assert!(status1.success());
    let status2 = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o2_path)
        .arg(&c2_path)
        .status()
        .unwrap();
    assert!(status2.success());

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "validate",
            "--bindings-json",
            bindings.to_str().unwrap(),
            "--artifact",
            o1_path.to_str().unwrap(),
            "--artifact",
            o2_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let matches = json["matches"].as_array().unwrap();
    assert!(matches.iter().any(|entry| {
        entry["name"] == "foo"
            && entry["status"] == "DuplicateProviders"
            && entry["provider_artifacts"].as_array().unwrap().len() == 2
    }));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_validate_reports_unresolved_declared_link_inputs() {
    let dir = temp_dir("validate_declared");
    let bindings = dir.join("bindings.json");
    let c_path = dir.join("lib.c");
    let o_path = dir.join("lib.o");

    std::fs::write(
        &bindings,
        serde_json::json!({
            "schema_version": 1,
            "bic_version": "0.1.0",
            "target": {},
            "inputs": {},
            "link": {
                "libraries": [
                    { "name": "demo", "kind": "Default", "source": "Declared" }
                ],
                "ordered_inputs": [
                    {
                        "Library": { "name": "demo", "kind": "Default", "source": "Declared" }
                    }
                ]
            },
            "source_path": "api.h",
            "items": [
                {
                    "Function": {
                        "name": "missing",
                        "calling_convention": "C",
                        "parameters": [],
                        "return_type": "Int",
                        "variadic": false,
                        "source_offset": null
                    }
                }
            ],
            "diagnostics": []
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(&c_path, "int foo(void) { return 7; }\n").unwrap();

    let status = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o_path)
        .arg(&c_path)
        .status()
        .unwrap();
    assert!(status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "validate",
            "--bindings-json",
            bindings.to_str().unwrap(),
            "--artifact",
            o_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let matches = json["matches"].as_array().unwrap();
    assert!(matches.iter().any(|entry| {
        entry["name"] == "missing"
            && entry["status"] == "UnresolvedDeclaredLinkInputs"
            && entry["provider_artifacts"].as_array().unwrap().is_empty()
    }));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_validate_reports_decoration_mismatch() {
    let dir = temp_dir("validate_decor");
    let bindings = dir.join("bindings.json");
    let c_path = dir.join("lib.c");
    let o_path = dir.join("lib.o");

    std::fs::write(
        &bindings,
        serde_json::json!({
            "schema_version": 1,
            "bic_version": "0.1.0",
            "target": {},
            "inputs": {},
            "link": {},
            "source_path": "api.h",
            "items": [
                {
                    "Function": {
                        "name": "foo",
                        "calling_convention": "C",
                        "parameters": [],
                        "return_type": "Int",
                        "variadic": false,
                        "source_offset": null
                    }
                }
            ],
            "diagnostics": []
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        &c_path,
        "int foo(void) __asm__(\"_foo\");\nint foo(void) { return 7; }\n",
    )
    .unwrap();

    let status = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&o_path)
        .arg(&c_path)
        .status()
        .unwrap();
    assert!(status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "validate",
            "--bindings-json",
            bindings.to_str().unwrap(),
            "--artifact",
            o_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let matches = json["matches"].as_array().unwrap();
    assert!(matches.iter().any(|entry| {
        entry["name"] == "foo" && entry["status"] == "DecorationMismatch"
    }));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_link_plan_emits_link_surface_json() {
    let dir = temp_dir("link_plan");
    let bindings = dir.join("bindings.json");

    std::fs::write(
        &bindings,
        serde_json::json!({
            "schema_version": 1,
            "bic_version": "0.1.0",
            "target": {},
            "inputs": {},
            "macros": [],
            "link": {
                "preferred_mode": "PreferDynamic",
                "native_surface_kind": "Mixed",
                "platform_constraints": ["macos", "aarch64"],
                "include_paths": ["include"],
                "framework_paths": ["/System/Library/Frameworks"],
                "library_paths": ["lib"],
                "frameworks": [
                    { "name": "Foundation", "source": "Declared" }
                ],
                "libraries": [
                    { "name": "ssl", "kind": "Dynamic", "source": "Inferred" }
                ],
                "artifacts": [
                    {
                        "path": "native/libcrypto.a",
                        "kind": "StaticLibrary",
                        "source": "Discovered"
                    }
                ],
                "ordered_inputs": [
                    { "Framework": { "name": "Foundation", "source": "Declared" } },
                    { "Library": { "name": "ssl", "kind": "Dynamic", "source": "Inferred" } },
                    {
                        "Artifact": {
                            "path": "native/libcrypto.a",
                            "kind": "StaticLibrary",
                            "source": "Discovered"
                        }
                    }
                ]
            },
            "source_path": "api.h",
            "items": [],
            "diagnostics": []
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args(["link-plan", "--bindings-json", bindings.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["preferred_mode"], "PreferDynamic");
    assert_eq!(json["native_surface_kind"], "Mixed");
    assert_eq!(json["platform_constraints"][0], "macos");
    assert_eq!(json["platform_constraints"][1], "aarch64");
    assert_eq!(json["include_paths"][0], "include");
    assert_eq!(json["framework_paths"][0], "/System/Library/Frameworks");
    assert_eq!(json["library_paths"][0], "lib");
    assert_eq!(json["frameworks"][0]["name"], "Foundation");
    assert_eq!(json["frameworks"][0]["source"], "Declared");
    assert_eq!(json["libraries"][0]["name"], "ssl");
    assert_eq!(json["libraries"][0]["kind"], "Dynamic");
    assert_eq!(json["libraries"][0]["source"], "Inferred");
    assert_eq!(json["artifacts"][0]["path"], "native/libcrypto.a");
    assert_eq!(json["artifacts"][0]["kind"], "StaticLibrary");
    assert_eq!(json["artifacts"][0]["source"], "Discovered");
    assert_eq!(json["ordered_inputs"].as_array().unwrap().len(), 3);
    assert_eq!(json["ordered_inputs"][0]["Framework"]["name"], "Foundation");
    assert_eq!(json["ordered_inputs"][1]["Library"]["name"], "ssl");
    assert_eq!(json["ordered_inputs"][2]["Artifact"]["path"], "native/libcrypto.a");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_probe_layout_emits_type_layout_json() {
    let dir = temp_dir("probe_layout");
    let header = dir.join("api.h");
    std::fs::write(
        &header,
        "typedef unsigned int value_t;\nstruct widget { int a; double b; };\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bic"))
        .args([
            "probe-layout",
            "--header",
            header.to_str().unwrap(),
            "--type",
            "value_t",
            "--type",
            "struct widget",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let layouts = json["layouts"].as_array().unwrap();
    assert!(layouts.iter().any(|layout| layout["name"] == "value_t"));
    assert!(layouts.iter().any(|layout| layout["name"] == "struct widget"));

    std::fs::remove_dir_all(&dir).ok();
}
