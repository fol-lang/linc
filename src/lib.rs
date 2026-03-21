//! LINC — link and binary evidence layer for C interop analysis.
//!
//! This crate is **LINC**: the
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
//! - source parsing and preprocessing (upstream frontend)
//! - source-level declaration extraction (upstream frontend)
//! - Rust FFI code generation (downstream: `gec`)
//!
//! # Preferred API
//!
//! New consumers should start with:
//!
//! - [`analyze_source_package`] to turn a frontend-neutral [`SourcePackage`]
//!   into a [`LinkAnalysisPackage`]
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
//! - [`raw_headers`]: Transitional raw-header bootstrap kept out of the normal API story
//! - [`diagnostics`]: Diagnostic types
//! - [`error`]: Error surface
//!
//! # Library Usage Guidance
//!
//! LINC is intentionally a library, not an executable product. The recommended
//! usage pattern is:
//!
//! 1. produce a [`SourcePackage`] in a frontend crate such as `parc`
//! 2. call [`analyze_source_package`] to obtain a [`LinkAnalysisPackage`]
//! 3. optionally enrich with layout evidence or symbol inventories
//! 4. serialize and hand the resulting values to downstream build/generation tooling
//!
//! # Current Error-Surface Inventory
//!
//! The long-term goal is a fully typed public error surface built around [`LincError`].
//! That work is not complete yet.
//!
//! Today, the remaining transitional operational APIs that still return string-based
//! errors are being migrated to the typed error surface.
//!
//! The preferred stability boundary today is:
//!
//! - typed data structures on success
//! - diagnostics carried in returned packages and reports
//! - `LincError` for JSON and schema transport concerns
//! - validation findings returned as structured report data
//!
//! Future plan slices will convert the remaining string-based operational failures
//! into typed crate errors.
//!
//! # Current Data-Contract Stability Guide
//!
//! For downstream consumers, the current package contract is best read with these rules:
//!
//! - identity/version fields such as `schema_version` and `linc_version` are contract-level metadata
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
//! - consumers should gate compatibility on `schema_version`, not `linc_version`
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
pub mod intake;
pub mod ir;
pub mod line_markers;
pub mod analysis;
pub mod link_plan;
pub mod probe;
#[doc(hidden)]
pub mod raw_headers;

#[cfg(feature = "symbols")]
pub mod symbols;
#[cfg(feature = "symbols")]
pub mod validate;

pub use diagnostics::{Diagnostic, DiagnosticKind, Severity};
pub use error::LincError;
pub use analysis::LinkAnalysisPackage;
pub use ir::SCHEMA_VERSION;
pub use intake::{
    SourceDeclaration, SourceEnum, SourceEnumVariant, SourceField, SourceFunction,
    SourceLinkKind, SourceLinkRequirement, SourceMacro, SourcePackage, SourceParameter,
    SourceRecord, SourceType, SourceTypeAlias, SourceVariable,
};
pub use link_plan::{
    resolve_link_plan, resolve_link_plan_for_target, resolve_link_plan_with_inventories,
    ProviderMatchKind, ProviderProvenance, RequirementResolution, ResolvedLinkPlan,
    ResolvedLinkRequirement, ResolvedProvider,
};
pub use probe::{
    probe_type_layouts, AbiProbeReport, ProbeConfig, ProbeConfidence, ProbeSubjectKind,
    ProbeSubjectReport, ProbedFieldLayout, RecordCompleteness,
};
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

/// Analyze a frontend-neutral [`SourcePackage`] into the explicit
/// [`LinkAnalysisPackage`] contract.
///
/// This is the preferred contract-first entrypoint for downstream consumers
/// that do not want to traffic in `BindingPackage`.
pub fn analyze_source_package(source: &SourcePackage) -> LinkAnalysisPackage {
    let binding = intake::adapters::to_binding_package(source);
    LinkAnalysisPackage::from_binding_package(&binding)
}

#[cfg(test)]
#[cfg(feature = "symbols")]
mod integration_tests {
    use super::*;

    #[test]
    fn validation_report_json_roundtrip() {
        let mut src_pkg = SourcePackage::default();
        src_pkg
            .declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "foo".into(),
                parameters: vec![],
                return_type: SourceType::Void,
                variadic: false,
                source_offset: None,
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "bar".into(),
                parameters: vec![],
                return_type: SourceType::Void,
                variadic: false,
                source_offset: None,
            }));
        let pkg = intake::adapters::to_binding_package(&src_pkg);
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

    #[test]
    fn downstream_consumer_pattern() {
        let mut src_pkg = SourcePackage::default();
        src_pkg
            .declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "init".into(),
                parameters: vec![SourceParameter {
                    name: Some("cfg".into()),
                    ty: SourceType::Pointer(Box::new(SourceType::Void)),
                }],
                return_type: SourceType::Int,
                variadic: false,
                source_offset: None,
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "shutdown".into(),
                parameters: vec![],
                return_type: SourceType::Void,
                variadic: false,
                source_offset: None,
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Record(SourceRecord {
                name: Some("config".into()),
                is_union: false,
                fields: Some(vec![
                    SourceField {
                        name: Some("flags".into()),
                        ty: SourceType::UInt,
                        bit_width: None,
                    },
                    SourceField {
                        name: Some("version".into()),
                        ty: SourceType::UInt,
                        bit_width: None,
                    },
                ]),
                source_offset: None,
            }));
        let package = intake::adapters::to_binding_package(&src_pkg);
        assert_eq!(package.function_count(), 2);
        assert!(package.find_function("init").is_some());
        assert!(package.find_function("shutdown").is_some());
        assert_eq!(package.record_count(), 1);
        let json = serde_json::to_string_pretty(&package).unwrap();
        assert!(json.contains("\"init\""));
        assert!(json.contains("\"config\""));
    }

    #[test]
    fn binding_package_json_roundtrip() {
        let pkg = intake::adapters::to_binding_package(&SourcePackage::default());
        assert_eq!(pkg.schema_version, SCHEMA_VERSION);
        let json = serde_json::to_string_pretty(&pkg).unwrap();
        let pkg2: ir::BindingPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg2.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn typed_error_matrix_for_public_operations() {
        let probe_no_headers =
            probe_type_layouts(&ProbeConfig::new(), &["struct widget"]).unwrap_err();
        assert!(matches!(probe_no_headers, LincError::NoHeaders));

        let probe_no_types =
            probe_type_layouts(&ProbeConfig::new().header("demo.h"), &[] as &[&str]).unwrap_err();
        assert!(matches!(probe_no_types, LincError::NoProbeTypes));

        let symbol_read = inspect_symbols("/nonexistent/path.o").unwrap_err();
        assert!(matches!(symbol_read, LincError::SymbolRead { .. }));
    }

    #[test]
    fn contract_snapshot_simple_api_package_is_consumable() {
        let json = include_str!("../tests/contracts/simple_api_package.json");
        let pkg: ir::BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.source_path.as_deref(), Some("demo.h"));
        assert_eq!(pkg.macros.len(), 1);
        assert_eq!(pkg.layouts.len(), 1);
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.items.len(), 2);
    }

    #[test]
    fn contract_snapshot_symbol_validation_fixture_is_consumable() {
        let json = include_str!("../tests/contracts/symbol_validation_fixture.json");
        let pkg: ir::BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.items.len(), 2);
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.link.ordered_inputs.len(), 1);
    }

    #[test]
    fn contract_snapshot_binding_package_contract_is_consumable() {
        let json = include_str!("../tests/contracts/binding_package_contract_snapshot.json");
        let pkg: ir::BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.source_path.as_deref(), Some("demo.h"));
        assert_eq!(pkg.macros.len(), 2);
        assert_eq!(pkg.macros[0].value, Some(ir::MacroValue::Integer(3)));
        assert_eq!(pkg.macros[1].form, ir::MacroForm::FunctionLike);
        assert_eq!(pkg.layouts.len(), 1);
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.items.len(), 1);
    }

    #[test]
    fn contract_snapshot_fol_minimal_contract_is_consumable() {
        let json = include_str!("../tests/contracts/fol_minimal_contract.json");
        let pkg: ir::BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.items.len(), 1);
        assert!(pkg.macros.is_empty());
        assert!(pkg.layouts.is_empty());
    }

    #[test]
    fn contract_snapshot_fol_extended_contract_is_consumable() {
        let json = include_str!("../tests/contracts/fol_extended_contract.json");
        let pkg: ir::BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.items.len(), 1);
        assert_eq!(pkg.macros.len(), 1);
        assert_eq!(pkg.macros[0].value, Some(ir::MacroValue::Integer(3)));
        assert_eq!(pkg.layouts.len(), 1);
        assert_eq!(pkg.link.libraries.len(), 1);
    }

    #[test]
    fn binding_package_json_invalid() {
        let result: Result<ir::BindingPackage, _> = serde_json::from_str("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn intake_source_package_to_binding_package() {
        let mut src_pkg = SourcePackage::default();
        src_pkg.source_path = Some("demo.h".into());
        src_pkg
            .declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "init".into(),
                parameters: vec![SourceParameter {
                    name: Some("flags".into()),
                    ty: SourceType::UInt,
                }],
                return_type: SourceType::Int,
                variadic: false,
                source_offset: None,
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Record(SourceRecord {
                name: Some("config".into()),
                is_union: false,
                fields: Some(vec![SourceField {
                    name: Some("version".into()),
                    ty: SourceType::UInt,
                    bit_width: None,
                }]),
                source_offset: None,
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Variable(SourceVariable {
                name: "errno".into(),
                ty: SourceType::Int,
                source_offset: None,
            }));
        src_pkg
            .link_requirements
            .push(intake::SourceLinkRequirement {
                name: "mylib".into(),
                kind: intake::source::SourceLinkKind::DynamicLibrary,
            });

        let pkg = intake::adapters::to_binding_package(&src_pkg);
        assert_eq!(pkg.source_path.as_deref(), Some("demo.h"));
        assert_eq!(pkg.function_count(), 1);
        assert_eq!(pkg.record_count(), 1);
        assert_eq!(pkg.variable_count(), 1);
        assert_eq!(pkg.find_function("init").unwrap().name, "init");
        assert_eq!(pkg.link.libraries.len(), 1);
        assert_eq!(pkg.link.libraries[0].name, "mylib");

        let json = serde_json::to_string_pretty(&pkg).unwrap();
        let pkg2: ir::BindingPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg, pkg2);
    }

    #[test]
    fn intake_roundtrip_preserves_all_declaration_types() {
        let mut src_pkg = SourcePackage::default();
        src_pkg
            .declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "foo".into(),
                parameters: vec![],
                return_type: SourceType::Void,
                variadic: false,
                source_offset: Some(10),
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Record(SourceRecord {
                name: Some("s".into()),
                is_union: false,
                fields: Some(vec![]),
                source_offset: Some(20),
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Enum(SourceEnum {
                name: Some("e".into()),
                variants: vec![intake::SourceEnumVariant {
                    name: "A".into(),
                    value: Some(0),
                }],
                source_offset: Some(30),
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::TypeAlias(SourceTypeAlias {
                name: "myint".into(),
                target: SourceType::Int,
                source_offset: Some(40),
            }));
        src_pkg
            .declarations
            .push(SourceDeclaration::Variable(SourceVariable {
                name: "var".into(),
                ty: SourceType::Int,
                source_offset: Some(50),
            }));

        let pkg = intake::adapters::to_binding_package(&src_pkg);
        assert_eq!(pkg.function_count(), 1);
        assert_eq!(pkg.record_count(), 1);
        assert_eq!(pkg.enum_count(), 1);
        assert_eq!(pkg.type_alias_count(), 1);
        assert_eq!(pkg.variable_count(), 1);
    }

    #[test]
    fn intake_to_validation_and_link_plan() {
        use crate::intake::SourcePackage;
        use crate::symbols::*;

        let src = SourcePackage {
            source_path: Some("api.h".into()),
            declarations: vec![
                SourceDeclaration::Function(SourceFunction {
                    name: "api_init".into(),
                    parameters: vec![],
                    return_type: SourceType::Int,
                    variadic: false,
                    source_offset: None,
                }),
                SourceDeclaration::Function(SourceFunction {
                    name: "api_shutdown".into(),
                    parameters: vec![],
                    return_type: SourceType::Void,
                    variadic: false,
                    source_offset: None,
                }),
                SourceDeclaration::Variable(SourceVariable {
                    name: "api_version".into(),
                    ty: SourceType::Int,
                    source_offset: None,
                }),
            ],
            ..SourcePackage::default()
        };

        let mut pkg = intake::adapters::to_binding_package(&src);
        pkg.link
            .ordered_inputs
            .push(ir::LinkInput::Library(ir::LinkLibrary {
                name: "api".into(),
                kind: ir::LinkLibraryKind::Default,
                source: ir::LinkRequirementSource::Declared,
            }));

        let inv = SymbolInventory {
            artifact_path: "/usr/lib/libapi.so".into(),
            format: ArtifactFormat::ElfSharedLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::SharedLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: vec!["libc.so.6".into()],
            symbols: vec![
                SymbolEntry {
                    name: "api_init".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
                    function_abi: None,
                },
                SymbolEntry {
                    name: "api_shutdown".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
                    function_abi: None,
                },
                SymbolEntry {
                    name: "api_version".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: Some(4),
                    section: None,
                    archive_member: None,
                    function_abi: None,
                },
            ],
        };

        let report = validate::validate(&pkg, &inv);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.matched, 3);
        assert_eq!(report.summary.missing, 0);
        assert!(report.all_matched());

        let plan = link_plan::resolve_link_plan_with_inventories(&pkg, std::slice::from_ref(&inv));
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(
            plan.requirements[0].resolution,
            link_plan::RequirementResolution::Resolved
        );
        assert_eq!(plan.transitive_dependencies, vec!["libc.so.6"]);

        let json = serde_json::to_string_pretty(&pkg).unwrap();
        let pkg2: ir::BindingPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg.item_count(), pkg2.item_count());
    }

}
