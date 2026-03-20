//! LINC — link and binary evidence layer for C interop analysis.
//!
//! This crate (currently named `bic` during migration) is becoming **LINC**: the
//! link-surface, symbol-inventory, validation, and ABI-evidence layer in the
//! `PARC → LINC → GERC` pipeline.
//!
//! # What LINC Owns
//!
//! - intake of normalized frontend/source contracts via [`SourcePackage`]
//! - binary symbol inspection via [`inspect_symbols`]
//! - object/shared-library/archive metadata extraction
//! - provider matching and link-plan construction
//! - ABI probe orchestration and retained measurement evidence
//! - declaration-vs-artifact validation via [`validate`]
//! - link and binary evidence reporting
//!
//! # What LINC Does Not Own
//!
//! - source parsing and preprocessing (upstream: `parc`)
//! - source-level declaration extraction (upstream: `parc`)
//! - Rust FFI code generation (downstream: `gec`)
//!
//! # Preferred API
//!
//! New consumers should start with:
//!
//! - [`from_source_package`] to ingest a frontend-neutral [`SourcePackage`]
//! - [`HeaderConfig`] for raw-header scanning (transitional, uses `parc` internally)
//! - [`BindingPackage`] and IR types for serialized machine contracts
//! - [`to_json`] and [`from_json`] for JSON transport
//! - [`probe_type_layouts`] for compiler-assisted ABI layout probing
//! - [`inspect_symbols`] for native artifact inventory
//! - [`validate`] and [`validate_many`] for declaration-vs-artifact validation
//!
//! # Module Organization
//!
//! - [`intake`]: Frontend-neutral source contract and adapters
//! - [`ir`]: LINC intermediate representation (split into link, types, macros)
//! - [`probe`]: ABI measurement and evidence
//! - [`symbols`]: Binary symbol inspection
//! - [`validate`]: Declaration-vs-artifact validation
//! - [`link_plan`]: Link-plan construction and resolution
//! - [`raw_headers`]: Header scanning (transitional)
//! - [`diagnostics`]: Diagnostic types
//! - [`error`]: Error surface
//!
//! # Library Usage Guidance
//!
//! LINC is intentionally a library, not an executable product. The recommended
//! usage pattern is:
//!
//! 1. produce a [`BindingPackage`] by scanning headers or parsing preprocessed input
//! 2. optionally enrich the package with layout evidence or symbol inventories
//! 3. serialize and hand the resulting values to downstream build/generation tooling
//!
//! # Current Error-Surface Inventory
//!
//! The long-term goal is a fully typed public error surface built around [`BicError`].
//! That work is not complete yet.
//!
//! Today, the remaining public APIs that still return `Result<_, String>` are:
//!
//! - [`extract_from_source`]
//! - [`HeaderConfig::process`]
//!
//! Those APIs are still usable, but downstream users should treat their exact error
//! strings as transitional rather than a stable matching contract.
//!
//! The preferred stability boundary today is:
//!
//! - typed data structures on success
//! - diagnostics carried in returned packages and reports
//! - `BicError` for JSON and schema transport concerns
//! - validation findings returned as structured report data
//!
//! Future plan slices will convert the remaining string-based operational failures
//! into typed crate errors.
//!
//! # Current Data-Contract Stability Guide
//!
//! For downstream consumers, the current package contract is best read with these rules:
//!
//! - identity/version fields such as `schema_version` and `bic_version` are contract-level metadata
//! - top-level package sections such as `target`, `inputs`, `macros`, `layouts`, `link`,
//!   `items`, and `diagnostics` are stable container concepts
//! - many nested fields are still best treated as additive/defaultable rather than frozen in
//!   their exact long-term shape
//! - validation and symbol inventories are structured evidence, not proof that every ABI detail
//!   is fully modeled
//! - `SCHEMA_VERSION` intentionally remains `1` for now because recent changes have been
//!   additive/defaultable rather than a reviewed breaking wire-format step
//!
//! Producer/consumer compatibility expectations are:
//!
//! - producers should prefer additive, defaultable growth over silent semantic rewrites
//! - consumers should gate compatibility on `schema_version`, not `bic_version`
//! - future schema versions should be rejected rather than guessed at
//!
//! # Current Failure Model
//!
//! The library currently distinguishes three kinds of outcomes:
//!
//! - hard operational failures returned as `Err(...)`
//! - successful analysis with diagnostics attached to returned data
//! - successful validation that may still report mismatches in structured result objects
//!
//! In practical terms:
//!
//! - transport and schema problems should be treated as hard errors
//! - source/toolchain issues that prevent useful analysis may return an error
//! - unsupported or partially represented source constructs should usually appear in
//!   `package.diagnostics`
//! - declaration-vs-artifact mismatches should appear in `ValidationReport`, not as thrown errors
//!
//! Consumers should therefore use a two-step acceptance model:
//!
//! 1. check whether the operation itself returned `Err(...)`
//! 2. if it succeeded, inspect diagnostics, layouts, and validation findings before
//!    treating the result as generation-ready
//!
pub mod diagnostics;
pub mod error;
pub(crate) mod extract;
pub mod intake;
pub mod ir;
pub mod line_markers;
pub mod link_plan;
pub(crate) mod preprocess;
pub mod probe;
pub mod raw_headers;

#[cfg(feature = "symbols")]
pub mod symbols;
#[cfg(feature = "symbols")]
pub mod validate;

pub use error::{BicError, LincError};
pub use diagnostics::{Diagnostic, DiagnosticKind, Severity};
pub use extract::{extract_from_source, extract_from_translation_unit};
pub use ir::{
    BindingDefine, BindingInputs, BindingItem, BindingItemKind, BindingLinkSurface, BindingPackage,
    AbiConfidence, AliasResolution, BindingTarget, BindingType, CallingConvention, DeclarationProvenance, EnumBinding,
    EnumRepresentation, EnumVariant, FieldBinding, FieldLayout, FunctionBinding, LinkArtifact, LinkArtifactKind, LinkFramework,
    LinkInput, LinkLibrary, LinkLibraryKind, LinkRequirementSource, LinkResolutionMode,
    MacroBinding, MacroCategory, MacroEnvironmentEntry, MacroForm, MacroKind, MacroProvenance,
    MacroValue,
    NativeSurfaceKind,
    ParameterBinding, RecordBinding, RecordKind, RecordRepresentation, TypeAliasBinding, TypeLayout, TypeQualifiers, UnsupportedItem,
    VariableBinding, SCHEMA_VERSION,
};
pub use line_markers::{FileOriginMap, OriginFilter, SourceLocation, SourceOrigin};
pub use link_plan::{
    resolve_link_plan, resolve_link_plan_for_target, resolve_link_plan_with_inventories,
    ProviderMatchKind, ProviderProvenance, RequirementResolution, ResolvedLinkPlan,
    ResolvedLinkRequirement, ResolvedProvider,
};
pub use preprocess::PreprocessedInput;
pub use probe::{
    probe_type_layouts, AbiProbeReport, ProbeConfidence, ProbeSubjectKind, ProbeSubjectReport,
    ProbedFieldLayout, RecordCompleteness,
};
pub use intake::{
    SourceDeclaration, SourceEnum, SourceEnumVariant, SourceField, SourceFunction,
    SourceLinkRequirement, SourceMacro, SourcePackage, SourceParameter, SourceRecord,
    SourceType, SourceTypeAlias, SourceVariable,
};
pub use raw_headers::{HeaderConfig, PreprocessingReport, RawHeaderResult};
#[cfg(feature = "symbols")]
pub use symbols::{
    inspect_file as inspect_symbols, FunctionAbiHint, SymbolBinding, SymbolDirection, SymbolEntry,
    SymbolInventory, SymbolVisibility,
};
#[cfg(feature = "symbols")]
pub use validate::{
    validate, validate_many, AbiShapeEvidence, EvidenceKind, FunctionMatch, ItemKind,
    MatchConfidence, MatchStatus, RoutineAbiConfidence, RoutineAbiEvidence, RoutineAbiEvidenceKind,
    SymbolMatch, ValidationDeclaration, ValidationEntry, ValidationEvidence, ValidationPhase,
    ValidationPhaseReport, ValidationReport, ValidationSummary,
};

/// Construct a [`BindingPackage`] from a frontend-neutral [`SourcePackage`].
///
/// This is the preferred intake path for LINC. Frontends should produce a
/// [`SourcePackage`] and pass it here instead of using parser-specific APIs.
pub fn from_source_package(source: &SourcePackage) -> BindingPackage {
    intake::adapters::to_binding_package(source)
}

/// Serialize a BindingPackage to a deterministic JSON string.
///
/// Compatibility notes:
///
/// - the serialized payload includes `schema_version`
/// - the semantic contract is the data model, not the exact whitespace layout
/// - additive fields should prefer serde defaults where backward compatibility is intended
pub fn to_json(package: &BindingPackage) -> Result<String, BicError> {
    serde_json::to_string_pretty(package).map_err(BicError::from)
}

/// Deserialize a BindingPackage from a JSON string.
///
/// Compatibility notes:
///
/// - older payloads that omit newer defaultable fields should deserialize successfully
/// - payloads with a future `schema_version` are rejected
/// - downstream users should treat `schema_version` as the compatibility gate, not
///   `bic_version`
///
/// Returns an error if the schema version is newer than what this version of BIC supports.
pub fn from_json(json: &str) -> Result<BindingPackage, BicError> {
    let pkg: BindingPackage = serde_json::from_str(json)?;
    if pkg.schema_version > ir::SCHEMA_VERSION {
        return Err(BicError::SchemaVersion {
            found: pkg.schema_version,
            supported: ir::SCHEMA_VERSION,
        });
    }
    Ok(pkg)
}

#[cfg(test)]
#[cfg(feature = "symbols")]
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

        // Step 3: Verify expected items
        assert!(pkg.find_type_alias("size_t").is_some());
        assert!(pkg.find_record("FILE").is_some());
        assert!(pkg.find_function("malloc").is_some());
        assert!(pkg.find_function("free").is_some());
        assert!(pkg.find_variable("errno").is_some());
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
            platform: symbols::ArtifactPlatform::Elf,
            kind: symbols::ArtifactKind::Object,
            capabilities: symbols::ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: None,
                version: None,
                direction: symbols::SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                function_abi: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: symbols::SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
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
    /// 3. (Optional) Validate against symbols
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

        // A downstream tool (like fol) would call LINC like this:
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
        assert!(json.contains("mul"));

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn schema_version_present_in_json() {
        let pkg = extract_from_source("void foo(void);").unwrap();
        let json = to_json(&pkg).unwrap();
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"bic_version\""));
    }

    #[test]
    fn schema_version_roundtrip() {
        let pkg = extract_from_source("void foo(void);").unwrap();
        assert_eq!(pkg.schema_version, SCHEMA_VERSION);
        let json = to_json(&pkg).unwrap();
        let pkg2 = from_json(&json).unwrap();
        assert_eq!(pkg2.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn reject_future_schema_version() {
        let json = r#"{"schema_version": 99, "bic_version": "0.1.0", "source_path": null, "items": [], "diagnostics": []}"#;
        let result = from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BicError::SchemaVersion { .. }));
    }

    #[test]
    fn accept_missing_schema_version_defaults_to_current() {
        // Old JSON without schema_version should deserialize with default
        let json = r#"{"source_path": null, "items": [], "diagnostics": []}"#;
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn contract_fixture_v0_minimal_binding_package_deserializes() {
        let json = include_str!("../test/contracts/v0_minimal_binding_package.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.schema_version, SCHEMA_VERSION);
        assert!(pkg.items.is_empty());
        assert!(pkg.diagnostics.is_empty());
    }

    #[test]
    fn contract_fixture_v1_empty_nested_objects_deserializes() {
        let json = include_str!("../test/contracts/v1_empty_nested_objects.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.schema_version, 1);
        assert!(pkg.target.target_triple.is_none());
        assert!(pkg.inputs.entry_headers.is_empty());
        assert!(pkg.link.libraries.is_empty());
    }

    #[test]
    fn schema_compatibility_matrix() {
        enum Expectation {
            OkCurrent,
            OkSchema(u32),
            FutureSchemaError,
        }

        let cases = [
            (
                "missing schema version defaults",
                r#"{"source_path": null, "items": [], "diagnostics": []}"#,
                Expectation::OkCurrent,
            ),
            (
                "v0 fixture",
                include_str!("../test/contracts/v0_minimal_binding_package.json"),
                Expectation::OkCurrent,
            ),
            (
                "v1 empty nested objects fixture",
                include_str!("../test/contracts/v1_empty_nested_objects.json"),
                Expectation::OkSchema(1),
            ),
            (
                "future schema rejected",
                r#"{"schema_version": 999, "bic_version": "0.1.0", "source_path": null, "items": [], "diagnostics": []}"#,
                Expectation::FutureSchemaError,
            ),
        ];

        for (label, json, expectation) in cases {
            match expectation {
                Expectation::OkCurrent => {
                    let pkg = from_json(json).unwrap_or_else(|err| {
                        panic!("{label} should deserialize successfully: {err}")
                    });
                    assert_eq!(pkg.schema_version, SCHEMA_VERSION, "{label}");
                }
                Expectation::OkSchema(expected) => {
                    let pkg = from_json(json).unwrap_or_else(|err| {
                        panic!("{label} should deserialize successfully: {err}")
                    });
                    assert_eq!(pkg.schema_version, expected, "{label}");
                }
                Expectation::FutureSchemaError => {
                    let err = from_json(json).unwrap_err();
                    assert!(
                        matches!(err, BicError::SchemaVersion { .. }),
                        "{label} should reject future schema versions"
                    );
                }
            }
        }
    }

    #[test]
    fn typed_error_matrix_for_public_operations() {
        let no_headers = HeaderConfig::new().process().unwrap_err();
        assert!(matches!(no_headers, BicError::NoHeaders));

        let probe_no_headers = probe_type_layouts(&HeaderConfig::new(), &["struct widget"])
            .unwrap_err();
        assert!(matches!(probe_no_headers, BicError::NoHeaders));

        let probe_no_types = probe_type_layouts(
            &HeaderConfig::new().header("demo.h"),
            &[] as &[&str],
        )
        .unwrap_err();
        assert!(matches!(probe_no_types, BicError::NoProbeTypes));

        let symbol_read = inspect_symbols("/nonexistent/path.o").unwrap_err();
        assert!(matches!(symbol_read, BicError::SymbolRead { .. }));

        let future_schema = from_json(
            r#"{"schema_version": 999, "bic_version": "0.1.0", "source_path": null, "items": [], "diagnostics": []}"#,
        )
        .unwrap_err();
        assert!(matches!(future_schema, BicError::SchemaVersion { .. }));
    }

    #[test]
    fn contract_snapshot_simple_api_package_is_consumable() {
        let json = include_str!("../test/contracts/simple_api_package.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.source_path.as_deref(), Some("demo.h"));
        assert_eq!(pkg.macros.len(), 1);
        assert_eq!(pkg.layouts.len(), 1);
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.items.len(), 2);
    }

    #[test]
    fn contract_snapshot_symbol_validation_fixture_is_consumable() {
        let json = include_str!("../test/contracts/symbol_validation_fixture.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.items.len(), 2);
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.link.ordered_inputs.len(), 1);
    }

    #[test]
    fn contract_snapshot_binding_package_contract_is_consumable() {
        let json = include_str!("../test/contracts/binding_package_contract_snapshot.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.source_path.as_deref(), Some("demo.h"));
        assert_eq!(pkg.macros.len(), 2);
        assert_eq!(pkg.macros[0].value, Some(MacroValue::Integer(3)));
        assert_eq!(pkg.macros[1].form, MacroForm::FunctionLike);
        assert_eq!(pkg.layouts.len(), 1);
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.items.len(), 1);
    }

    #[test]
    fn contract_snapshot_fol_minimal_contract_is_consumable() {
        let json = include_str!("../test/contracts/fol_minimal_contract.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.items.len(), 1);
        assert!(pkg.macros.is_empty());
        assert!(pkg.layouts.is_empty());
    }

    #[test]
    fn contract_snapshot_fol_extended_contract_is_consumable() {
        let json = include_str!("../test/contracts/fol_extended_contract.json");
        let pkg = from_json(json).unwrap();
        assert_eq!(pkg.items.len(), 1);
        assert_eq!(pkg.macros.len(), 1);
        assert_eq!(pkg.macros[0].value, Some(MacroValue::Integer(3)));
        assert_eq!(pkg.layouts.len(), 1);
        assert_eq!(pkg.link.libraries.len(), 1);
    }

    // Phase 25: error path and edge case tests

    #[test]
    fn parse_invalid_c_returns_error() {
        let result = extract_from_source("@#$% not valid C");
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_input() {
        let pkg = extract_from_source("").unwrap();
        assert!(pkg.items.is_empty());
    }

    #[test]
    fn deep_pointer_chain() {
        let pkg = extract_from_source("void foo(int *****p);").unwrap();
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                let mut ty = &f.parameters[0].ty;
                let mut depth = 0;
                while let BindingType::Pointer { pointee, .. } = ty {
                    depth += 1;
                    ty = pointee;
                }
                assert_eq!(depth, 5);
                assert_eq!(*ty, BindingType::Int);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn empty_struct() {
        // GCC extension: empty struct
        let pkg = extract_from_source("struct empty {};").unwrap();
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1);
        assert!(!records[0].is_opaque());
        assert!(records[0].fields.as_ref().unwrap().is_empty());
    }

    #[test]
    fn zero_length_array() {
        let pkg = extract_from_source("struct buf { int len; int data[0]; };").unwrap();
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        let fields = records[0].fields.as_ref().unwrap();
        assert_eq!(fields.len(), 2);
        match &fields[1].ty {
            BindingType::Array(_, Some(0)) => {}
            other => panic!("expected Array(_, Some(0)), got {:?}", other),
        }
    }

    #[test]
    fn determinism_10_runs() {
        let src = "typedef int int32_t; void foo(int32_t x); struct s { int a; float b; };";
        let first = to_json(&extract_from_source(src).unwrap()).unwrap();
        for _ in 0..9 {
            let json = to_json(&extract_from_source(src).unwrap()).unwrap();
            assert_eq!(first, json, "non-deterministic output detected");
        }
    }

    #[test]
    fn roundtrip_all_type_variants() {
        let src = r#"
            typedef int myint;
            typedef void (*cb)(int);
            enum e { A = 0 };
            struct s { int x; };
            union u { int i; float f; };
            void func(const char *s, int arr[10], ...);
            extern int var;
        "#;
        let pkg = extract_from_source(src).unwrap();
        let json = to_json(&pkg).unwrap();
        let pkg2 = from_json(&json).unwrap();
        let json2 = to_json(&pkg2).unwrap();
        assert_eq!(json, json2, "roundtrip produced different JSON");
    }

    #[test]
    fn from_json_invalid_json() {
        let result = from_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn intake_source_package_to_binding_package() {
        let mut src_pkg = SourcePackage::default();
        src_pkg.source_path = Some("demo.h".into());
        src_pkg.declarations.push(SourceDeclaration::Function(SourceFunction {
            name: "init".into(),
            parameters: vec![SourceParameter {
                name: Some("flags".into()),
                ty: SourceType::UInt,
            }],
            return_type: SourceType::Int,
            variadic: false,
            source_offset: None,
        }));
        src_pkg.declarations.push(SourceDeclaration::Record(SourceRecord {
            name: Some("config".into()),
            is_union: false,
            fields: Some(vec![SourceField {
                name: Some("version".into()),
                ty: SourceType::UInt,
                bit_width: None,
            }]),
            source_offset: None,
        }));
        src_pkg.declarations.push(SourceDeclaration::Variable(SourceVariable {
            name: "errno".into(),
            ty: SourceType::Int,
            source_offset: None,
        }));
        src_pkg.link_requirements.push(intake::SourceLinkRequirement {
            name: "mylib".into(),
            kind: intake::source::SourceLinkKind::DynamicLibrary,
        });

        let pkg = from_source_package(&src_pkg);
        assert_eq!(pkg.source_path.as_deref(), Some("demo.h"));
        assert_eq!(pkg.function_count(), 1);
        assert_eq!(pkg.record_count(), 1);
        assert_eq!(pkg.variable_count(), 1);
        assert_eq!(pkg.find_function("init").unwrap().name, "init");
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.link.libraries[0].name, "mylib");

        // JSON roundtrip
        let json = to_json(&pkg).unwrap();
        let pkg2 = from_json(&json).unwrap();
        assert_eq!(pkg, pkg2);
    }

    #[test]
    fn intake_roundtrip_preserves_all_declaration_types() {
        let mut src_pkg = SourcePackage::default();
        src_pkg.declarations.push(SourceDeclaration::Function(SourceFunction {
            name: "foo".into(),
            parameters: vec![],
            return_type: SourceType::Void,
            variadic: false,
            source_offset: Some(10),
        }));
        src_pkg.declarations.push(SourceDeclaration::Record(SourceRecord {
            name: Some("s".into()),
            is_union: false,
            fields: Some(vec![]),
            source_offset: Some(20),
        }));
        src_pkg.declarations.push(SourceDeclaration::Enum(SourceEnum {
            name: Some("e".into()),
            variants: vec![intake::SourceEnumVariant {
                name: "A".into(),
                value: Some(0),
            }],
            source_offset: Some(30),
        }));
        src_pkg.declarations.push(SourceDeclaration::TypeAlias(SourceTypeAlias {
            name: "myint".into(),
            target: SourceType::Int,
            source_offset: Some(40),
        }));
        src_pkg.declarations.push(SourceDeclaration::Variable(SourceVariable {
            name: "var".into(),
            ty: SourceType::Int,
            source_offset: Some(50),
        }));

        let pkg = from_source_package(&src_pkg);
        assert_eq!(pkg.function_count(), 1);
        assert_eq!(pkg.record_count(), 1);
        assert_eq!(pkg.enum_count(), 1);
        assert_eq!(pkg.type_alias_count(), 1);
        assert_eq!(pkg.variable_count(), 1);
    }

}
