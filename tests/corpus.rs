//! Real-world corpus tests for LINC.
//!
//! Uses vendored headers from testdata/full_apps/external/ (copied from parc).
//! System-header tests (string.h, symbol validation) are #[ignore] and
//! require gcc/clang or dev libraries.

use std::path::{Path, PathBuf};

use linc::*;

/// Path to the vendored test corpus.
fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("full_apps")
        .join("external")
}

fn find_system_header(name: &str) -> Option<PathBuf> {
    let paths = [
        format!("/usr/include/{}", name),
        format!("/usr/local/include/{}", name),
        format!("/usr/include/x86_64-linux-gnu/{}", name),
    ];
    paths
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

fn find_system_lib(name: &str) -> Option<PathBuf> {
    let paths = [
        format!("/usr/lib/{}", name),
        format!("/usr/lib/x86_64-linux-gnu/{}", name),
        format!("/usr/local/lib/{}", name),
        format!("/lib/x86_64-linux-gnu/{}", name),
    ];
    paths
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

// ============================================================================
// zlib corpus — uses vendored headers, no system deps
// ============================================================================

#[test]
fn zlib_vendored_parse() {
    let zlib_inc = corpus_dir().join("zlib/header/include");
    let main_c = corpus_dir().join("zlib/header/main.c");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&main_c)
        .include_dir(&zlib_inc)
        .no_origin_filter() // include all declarations
        .process()
        .unwrap();

    let funcs: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    // Key zlib functions should be present
    assert!(funcs.contains(&"deflate"), "missing deflate");
    assert!(funcs.contains(&"inflate"), "missing inflate");
    assert!(funcs.contains(&"compress"), "missing compress");
    assert!(funcs.contains(&"uncompress"), "missing uncompress");
    assert!(
        funcs.len() >= 30,
        "expected at least 30 zlib functions, got {}",
        funcs.len()
    );
}

#[test]
fn zlib_vendored_origin_filter() {
    let zlib_inc = corpus_dir().join("zlib/header/include");
    let zlib_h = zlib_inc.join("zlib.h");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&zlib_h)
        .include_dir(&zlib_inc)
        .process() // default filter: exclude system
        .unwrap();

    let funcs: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    assert!(funcs.contains(&"deflate"), "missing deflate");
    assert!(funcs.contains(&"inflate"), "missing inflate");

    // System functions should be filtered out (if any leaked in)
    assert!(!funcs.contains(&"printf"), "printf should be filtered out");
}

#[test]
fn zlib_vendored_types() {
    let zlib_inc = corpus_dir().join("zlib/header/include");
    let zlib_h = zlib_inc.join("zlib.h");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&zlib_h)
        .include_dir(&zlib_inc)
        .no_origin_filter()
        .process()
        .unwrap();

    let type_names: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::TypeAlias(t) => Some(t.name.as_str()),
            _ => None,
        })
        .collect();

    assert!(
        type_names.contains(&"Bytef") || type_names.contains(&"uLong") || type_names.contains(&"z_stream"),
        "expected zlib typedefs, got: {:?}",
        type_names
    );
}

#[test]
fn zlib_vendored_package_and_json_roundtrip() {
    let zlib_inc = corpus_dir().join("zlib/header/include");
    let zlib_h = zlib_inc.join("zlib.h");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&zlib_h)
        .include_dir(&zlib_inc)
        .no_origin_filter()
        .process()
        .unwrap();

    assert!(result.package.find_function("deflate").is_some(), "expected deflate function");
    assert!(result.package.find_function("inflate").is_some(), "expected inflate function");

    // JSON roundtrip
    let json = linc::to_json(&result.package).unwrap();
    let pkg2 = linc::from_json(&json).unwrap();
    assert_eq!(result.package, pkg2);
}

#[test]
fn zlib_vendored_determinism() {
    let zlib_inc = corpus_dir().join("zlib/header/include");
    let zlib_h = zlib_inc.join("zlib.h");

    let r1 = linc::raw_headers::HeaderConfig::new()
        .header(&zlib_h)
        .include_dir(&zlib_inc)
        .no_origin_filter()
        .process()
        .unwrap();
    let r2 = linc::raw_headers::HeaderConfig::new()
        .header(&zlib_h)
        .include_dir(&zlib_inc)
        .no_origin_filter()
        .process()
        .unwrap();

    let json1 = linc::to_json(&r1.package).unwrap();
    let json2 = linc::to_json(&r2.package).unwrap();
    assert_eq!(json1, json2, "JSON output should be deterministic");
}

// ============================================================================
// libpng corpus — uses vendored headers
// ============================================================================

#[test]
fn libpng_vendored_parse() {
    let png_inc = corpus_dir().join("libpng/header/include");
    let main_c = corpus_dir().join("libpng/header/main.c");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&main_c)
        .include_dir(&png_inc)
        .no_origin_filter()
        .process()
        .unwrap();

    let funcs: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    assert!(
        funcs.iter().any(|f| f.starts_with("png_")),
        "expected png_ functions, got: {:?}",
        &funcs[..funcs.len().min(10)]
    );
}

#[test]
fn libpng_vendored_package_inspection() {
    let png_inc = corpus_dir().join("libpng/header/include");
    let png_h = png_inc.join("png.h");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&png_h)
        .include_dir(&png_inc)
        .no_origin_filter()
        .process()
        .unwrap();

    // Should have at least some png_ functions in the extracted package
    let has_png_func = result.package.functions().any(|f| f.name.starts_with("png_"));
    assert!(has_png_func, "expected png_ functions in extracted package");
}

// ============================================================================
// musl stdint corpus — uses vendored headers
// ============================================================================

#[test]
fn musl_stdint_vendored_parse() {
    let musl_inc = corpus_dir().join("musl/stdint/include");
    let main_c = corpus_dir().join("musl/stdint/main.c");

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&main_c)
        .include_dir(&musl_inc)
        .no_origin_filter()
        .process()
        .unwrap();

    let type_names: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::TypeAlias(t) => Some(t.name.as_str()),
            _ => None,
        })
        .collect();

    // Key stdint types
    let expected = ["int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t"];
    for name in &expected {
        assert!(
            type_names.contains(name),
            "missing typedef {}, got: {:?}",
            name,
            type_names
        );
    }
}

// ============================================================================
// System header tests — require gcc/clang + system headers
// ============================================================================

#[test]
#[ignore] // Requires gcc/clang
fn string_h_parse() {
    let header = match find_system_header("string.h") {
        Some(p) => p,
        None => return,
    };

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&header)
        .process()
        .unwrap();

    let funcs: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    let expected = ["memcpy", "memset", "strlen", "strcmp", "memmove", "memcmp"];
    for name in &expected {
        assert!(funcs.contains(name), "missing {}", name);
    }
}

#[test]
#[ignore] // Requires gcc/clang
fn string_h_const_correctness() {
    let header = match find_system_header("string.h") {
        Some(p) => p,
        None => return,
    };

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&header)
        .process()
        .unwrap();

    let strlen = result
        .package
        .items
        .iter()
        .find_map(|i| match i {
            BindingItem::Function(f) if f.name == "strlen" => Some(f),
            _ => None,
        });

    if let Some(f) = strlen {
        assert_eq!(f.parameters.len(), 1);
        assert_eq!(
            f.parameters[0].ty,
            BindingType::const_ptr(BindingType::Char),
            "strlen parameter should be const char *"
        );
    }

    let memcpy = result
        .package
        .items
        .iter()
        .find_map(|i| match i {
            BindingItem::Function(f) if f.name == "memcpy" => Some(f),
            _ => None,
        });

    if let Some(f) = memcpy {
        assert!(f.parameters.len() >= 3);
        assert_eq!(
            f.parameters[0].ty,
            BindingType::Pointer {
                pointee: Box::new(BindingType::Void),
                const_pointee: false,
                qualifiers: linc::TypeQualifiers {
                    is_const: false,
                    is_volatile: false,
                    is_restrict: true,
                    is_atomic: false,
                },
            },
            "memcpy dest should be void *"
        );
        assert_eq!(
            f.parameters[1].ty,
            BindingType::Pointer {
                pointee: Box::new(BindingType::Void),
                const_pointee: true,
                qualifiers: linc::TypeQualifiers {
                    is_const: false,
                    is_volatile: false,
                    is_restrict: true,
                    is_atomic: false,
                },
            },
            "memcpy src should be const void *"
        );
    }
}

// ============================================================================
// Symbol validation — requires system libs
// ============================================================================

#[test]
#[ignore] // Requires zlib1g-dev: sudo apt install zlib1g-dev
fn zlib_system_parse_filtered() {
    let header = match find_system_header("zlib.h") {
        Some(p) => p,
        None => return,
    };

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&header)
        .process()
        .unwrap();

    let funcs: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    // Origin filtering should keep only zlib declarations
    assert!(funcs.contains(&"deflate"), "missing deflate");
    assert!(funcs.contains(&"inflate"), "missing inflate");
    assert!(funcs.contains(&"compress"), "missing compress");
    assert!(funcs.contains(&"uncompress"), "missing uncompress");
    assert!(!funcs.contains(&"printf"), "system function leaked through filter");

    eprintln!("zlib system: {} functions extracted", funcs.len());

    // Verify key functions extracted
    assert!(result.package.find_function("deflate").is_some(), "expected deflate");
    assert!(result.package.find_function("inflate").is_some(), "expected inflate");

    // JSON roundtrip
    let json = linc::to_json(&result.package).unwrap();
    let pkg2 = linc::from_json(&json).unwrap();
    assert_eq!(result.package, pkg2);
}

#[test]
#[ignore] // Requires libpng-dev: sudo apt install libpng-dev
fn libpng_system_parse() {
    let header = match find_system_header("png.h") {
        Some(p) => p,
        None => return,
    };

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&header)
        .process()
        .unwrap();

    let funcs: Vec<&str> = result
        .package
        .items
        .iter()
        .filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    assert!(
        funcs.iter().any(|f| f.starts_with("png_")),
        "expected png_ functions"
    );
    assert!(!funcs.contains(&"printf"), "system function leaked through filter");

    eprintln!("libpng system: {} functions extracted", funcs.len());
}

#[test]
#[ignore] // Requires libpng-dev: sudo apt install libpng-dev
fn libpng_system_validate_symbols() {
    let header = match find_system_header("png.h") {
        Some(p) => p,
        None => return,
    };
    let lib = match find_system_lib("libpng16.so")
        .or_else(|| find_system_lib("libpng.so"))
    {
        Some(p) => p,
        None => return,
    };

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&header)
        .process()
        .unwrap();

    let inventory = inspect_symbols(&lib).unwrap();
    let report = validate(&result.package, &inventory);

    let matched = report.matched().len();
    let total = report.matches.len();
    assert!(matched > 0, "expected some matched symbols, got 0 of {}", total);

    eprintln!(
        "libpng validation: {}/{} matched, {} missing",
        matched, total, report.missing().len()
    );
}

#[test]
#[ignore] // Requires zlib1g-dev: sudo apt install zlib1g-dev
fn zlib_system_validate_symbols() {
    let header = match find_system_header("zlib.h") {
        Some(p) => p,
        None => return,
    };
    let lib = match find_system_lib("libz.so") {
        Some(p) => p,
        None => return,
    };

    let result = linc::raw_headers::HeaderConfig::new()
        .header(&header)
        .process()
        .unwrap();

    let inventory = inspect_symbols(&lib).unwrap();
    let report = validate(&result.package, &inventory);

    let matched = report.matched().len();
    let total = report.matches.len();
    assert!(matched > 0, "expected some matched symbols, got 0 of {}", total);

    eprintln!(
        "zlib validation: {}/{} matched, {} missing",
        matched, total, report.missing().len()
    );
}
