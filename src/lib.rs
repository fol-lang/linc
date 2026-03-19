pub mod codegen_rust;
pub mod diagnostics;
pub mod extract;
pub mod ir;
pub mod line_markers;
pub mod preprocess;
pub mod raw_headers;
pub mod symbols;
pub mod validate;

pub use codegen_rust::emit_rust_ffi;
pub use diagnostics::{Diagnostic, DiagnosticKind, Severity};
pub use extract::{extract_from_source, extract_from_translation_unit};
pub use ir::{
    BindingItem, BindingPackage, BindingType, CallingConvention, EnumBinding, EnumVariant,
    FieldBinding, FunctionBinding, ParameterBinding, RecordBinding, RecordKind, TypeAliasBinding,
    UnsupportedItem, VariableBinding,
};
pub use line_markers::{FileOriginMap, OriginFilter, SourceOrigin};
pub use preprocess::PreprocessedInput;
pub use raw_headers::{HeaderConfig, PreprocessingReport, RawHeaderResult};
pub use symbols::{
    inspect_file as inspect_symbols, SymbolBinding, SymbolEntry, SymbolInventory, SymbolVisibility,
};
pub use validate::{validate, FunctionMatch, ItemKind, MatchStatus, SymbolMatch, ValidationReport};

/// Serialize a BindingPackage to a deterministic JSON string.
pub fn to_json(package: &BindingPackage) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(package)
}

/// Deserialize a BindingPackage from a JSON string.
pub fn from_json(json: &str) -> Result<BindingPackage, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn full_pipeline_preprocessed() {
        let src = r#"
            typedef unsigned long size_t;
            struct FILE;
            enum status { OK = 0, ERR = 1 };
            void *malloc(size_t n);
            void free(void *ptr);
            extern int errno;
        "#;

        // Step 1: Parse and extract
        let pkg = extract_from_source(src).unwrap();
        assert!(pkg.diagnostics.is_empty());

        // Step 2: Serialize to JSON and back
        let json = to_json(&pkg).unwrap();
        let pkg2 = from_json(&json).unwrap();
        assert_eq!(pkg, pkg2);

        // Step 3: Emit Rust FFI
        let rust = emit_rust_ffi(&pkg);
        assert!(rust.contains("pub type size_t"));
        assert!(rust.contains("pub struct FILE"));
        assert!(rust.contains("pub fn malloc"));
        assert!(rust.contains("pub fn free"));
        assert!(rust.contains("pub static errno"));
    }

    #[test]
    fn full_pipeline_preprocessed_input() {
        let pkg = PreprocessedInput::from_string("int foo(int x);")
            .with_path("test.i")
            .extract();

        assert_eq!(pkg.source_path.as_deref(), Some("test.i"));
        assert_eq!(pkg.items.len(), 1);

        let json = to_json(&pkg).unwrap();
        assert!(json.contains("foo"));
    }

    #[test]
    fn json_roundtrip_preserves_all_item_types() {
        let src = r#"
            typedef int int32_t;
            enum color { RED = 0, GREEN = 1 };
            struct point { int x; int y; };
            union data { int i; float f; };
            void func(int a, ...);
            extern int global_var;
        "#;
        let pkg = extract_from_source(src).unwrap();
        let json = to_json(&pkg).unwrap();
        let pkg2 = from_json(&json).unwrap();

        // Verify each item type survived
        assert!(pkg2.items.iter().any(|i| matches!(i, BindingItem::TypeAlias(_))));
        assert!(pkg2.items.iter().any(|i| matches!(i, BindingItem::Enum(_))));
        assert!(pkg2.items.iter().any(|i| matches!(i, BindingItem::Record(r) if r.kind == RecordKind::Struct)));
        assert!(pkg2.items.iter().any(|i| matches!(i, BindingItem::Record(r) if r.kind == RecordKind::Union)));
        assert!(pkg2.items.iter().any(|i| matches!(i, BindingItem::Function(_))));
        assert!(pkg2.items.iter().any(|i| matches!(i, BindingItem::Variable(_))));
    }

    #[test]
    fn validation_report_json_roundtrip() {
        let pkg = extract_from_source("void foo(void); void bar(void);").unwrap();
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: symbols::ArtifactFormat::ElfObject,
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: symbols::SymbolBinding::Global,
                size: None,
                section: None,
            }],
        };
        let report = validate(&pkg, &inv);
        let json = serde_json::to_string_pretty(&report).unwrap();
        let report2: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, report2);
        assert_eq!(report2.matched().len(), 1);
        assert_eq!(report2.missing().len(), 1);
    }

    /// Demonstrates the downstream consumer pattern described in PLAN.md:
    /// 1. Parse headers -> BindingPackage
    /// 2. Serialize to JSON for machine consumption
    /// 3. Emit Rust FFI
    /// 4. (Optional) Validate against symbols
    #[test]
    fn downstream_consumer_pattern() {
        let headers = r#"
            typedef unsigned int uint32_t;
            struct config {
                uint32_t flags;
                uint32_t version;
            };
            int init(struct config *cfg);
            void shutdown(void);
        "#;

        // A downstream tool (like fol) would call bic like this:
        let package = extract_from_source(headers).unwrap();

        // Inspect the package programmatically
        let functions: Vec<&FunctionBinding> = package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f),
            _ => None,
        }).collect();
        assert_eq!(functions.len(), 2);
        assert!(functions.iter().any(|f| f.name == "init"));
        assert!(functions.iter().any(|f| f.name == "shutdown"));

        let records: Vec<&RecordBinding> = package.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name.as_deref(), Some("config"));

        // Export as JSON for other tooling
        let json = to_json(&package).unwrap();
        assert!(json.contains("\"init\""));
        assert!(json.contains("\"config\""));

        // Generate Rust FFI
        let rust_ffi = emit_rust_ffi(&package);
        assert!(rust_ffi.contains("pub fn init"));
        assert!(rust_ffi.contains("pub fn shutdown"));
        assert!(rust_ffi.contains("pub struct config"));
    }

    #[test]
    #[ignore] // Requires gcc/clang and cc/ar
    fn full_end_to_end_with_raw_headers_and_symbols() {
        let dir = std::env::temp_dir().join("bic_e2e_test");
        std::fs::create_dir_all(&dir).unwrap();

        // Write a header and implementation
        let h_path = dir.join("mylib.h");
        let c_path = dir.join("mylib.c");
        let o_path = dir.join("mylib.o");

        std::fs::write(&h_path, "int add(int a, int b);\nint mul(int a, int b);\n").unwrap();
        std::fs::write(&c_path, "#include \"mylib.h\"\nint add(int a, int b) { return a+b; }\nint mul(int a, int b) { return a*b; }\n").unwrap();

        // Compile
        let status = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&o_path)
            .arg(&c_path)
            .status()
            .unwrap();
        assert!(status.success());

        // Step 1: Parse raw headers
        let result = HeaderConfig::new()
            .header(&h_path)
            .process()
            .unwrap();
        let package = result.package;

        // Step 2: Inspect symbols
        let inventory = inspect_symbols(&o_path).unwrap();

        // Step 3: Validate
        let report = validate(&package, &inventory);
        assert!(report.all_matched());

        // Step 4: JSON export
        let json = to_json(&package).unwrap();
        assert!(json.contains("add"));

        // Step 5: Rust FFI
        let rust = emit_rust_ffi(&package);
        assert!(rust.contains("pub fn add"));
        assert!(rust.contains("pub fn mul"));

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }
}
