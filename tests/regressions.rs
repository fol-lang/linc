use bic::{
    from_json, resolve_link_plan_for_target, validate, AbiProbeReport, BindingItem,
    BindingPackage, BindingType, CallingConvention, FunctionBinding, LinkInput, LinkLibrary,
    LinkLibraryKind, LinkRequirementSource, MacroValue, ParameterBinding, SymbolInventory,
    ValidationReport,
};
use bic::symbols::{ArtifactCapabilities, ArtifactFormat, ArtifactKind, ArtifactPlatform};
use std::path::PathBuf;

#[test]
fn regression_old_json_with_empty_nested_objects_stays_consumable() {
    let pkg = from_json(include_str!("../test/contracts/v1_empty_nested_objects.json")).unwrap();
    assert_eq!(pkg.schema_version, bic::SCHEMA_VERSION);
    assert!(pkg.inputs.entry_headers.is_empty());
    assert!(pkg.link.ordered_inputs.is_empty());
}

#[test]
fn regression_target_filtered_link_plan_keeps_transitive_edges() {
    let mut package = BindingPackage::new();
    package.link.platform_constraints.push("linux".into());
    package.link.ordered_inputs.push(LinkInput::Library(LinkLibrary {
        name: "z".into(),
        kind: LinkLibraryKind::Default,
        source: LinkRequirementSource::Declared,
    }));
    let inventories = vec![SymbolInventory {
        artifact_path: "/usr/lib/libz.so".into(),
        format: ArtifactFormat::ElfSharedLibrary,
        platform: ArtifactPlatform::Elf,
        kind: ArtifactKind::SharedLibrary,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: true,
        },
        dependency_edges: vec!["libc.so.6".into()],
        symbols: Vec::new(),
    }];

    let plan =
        resolve_link_plan_for_target(&package, &inventories, Some("x86_64-unknown-linux-gnu"));
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(plan.transitive_dependencies, vec!["libc.so.6"]);
}

#[test]
fn regression_decorated_fixture_validates_against_normalized_name() {
    let package = BindingPackage {
        items: vec![BindingItem::Function(FunctionBinding {
            name: "demo_init".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![ParameterBinding {
                name: Some("ctx".into()),
                ty: BindingType::ptr(BindingType::Void),
            }],
            return_type: BindingType::Int,
            variadic: false,
            source_offset: None,
        })],
        ..BindingPackage::new()
    };
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../test/contracts/decorated_symbol_inventory_fixture.json"
    ))
    .unwrap();

    let report = validate(&package, &inventory);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].status, bic::MatchStatus::Matched);
}

#[test]
fn regression_extended_contract_fixture_keeps_macro_value() {
    let pkg = from_json(include_str!("../test/contracts/fol_extended_contract.json")).unwrap();
    assert_eq!(pkg.macros[0].value, Some(MacroValue::Integer(3)));
}

#[test]
fn regression_probe_record_fixture_keeps_record_and_enum_metadata() {
    let report: AbiProbeReport = serde_json::from_str(include_str!(
        "../test/contracts/probe_record_contract_snapshot.json"
    ))
    .unwrap();
    assert_eq!(report.subjects.len(), 2);
    assert_eq!(report.subjects[0].layout.name, "struct widget");
    assert_eq!(report.subjects[1].enum_underlying_size, Some(4));
}

#[test]
fn regression_duplicate_provider_report_fixture_stays_consumable() {
    let report: ValidationReport = serde_json::from_str(include_str!(
        "../test/contracts/validation_duplicate_provider_report.json"
    ))
    .unwrap();
    assert_eq!(report.summary.duplicate_providers, 1);
    assert_eq!(report.duplicate_providers().len(), 1);
    assert_eq!(report.entries[0].evidence.evidence_kind, bic::EvidenceKind::DuplicateVisibleProviders);
}

#[test]
fn regression_function_abi_questionable_fixture_stays_consumable() {
    let report: ValidationReport = serde_json::from_str(include_str!(
        "../test/contracts/function_abi_questionable_report.json"
    ))
    .unwrap();
    assert_eq!(report.summary.abi_shape_mismatches, 1);
    assert_eq!(report.matches[0].status, bic::MatchStatus::AbiShapeMismatch);
    let routine = report.entries[0].evidence.routine_abi.as_ref().unwrap();
    assert_eq!(routine.evidence_kind, Some(bic::RoutineAbiEvidenceKind::Mismatch));
    assert_eq!(routine.confidence, Some(bic::RoutineAbiConfidence::Mismatch));
    assert_eq!(routine.expected_parameter_sizes, vec![Some(8), Some(4)]);
    assert_eq!(routine.observed_parameter_sizes, vec![Some(8), Some(8)]);
}

#[test]
fn regression_tricky_layout_fixture_stays_consumable() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/fixtures/tricky_layouts.h");
    let result = bic::HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("struct packed_flags")
        .probe_type_layout("enum widget_mode")
        .process()
        .unwrap();

    let alias = result.package.find_type_alias("my_size_ptr").unwrap();
    let resolution = alias.canonical_resolution.as_ref().unwrap();
    assert_eq!(resolution.alias_chain, vec!["my_size_t", "size_t"]);

    let record = result.package.find_record("packed_flags").unwrap();
    assert_eq!(record.abi_confidence, Some(bic::AbiConfidence::PartialBitfieldLayout));
    let fields = record.fields.as_ref().unwrap();
    assert_eq!(fields[0].bit_width, Some(3));
    assert_eq!(fields[1].bit_width, Some(5));

    let widget_mode = result.package.find_enum("widget_mode").unwrap();
    assert_eq!(
        widget_mode.abi_confidence,
        Some(bic::AbiConfidence::RepresentationProbed)
    );
}

#[test]
fn regression_tricky_macro_fixture_stays_consumable() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/fixtures/tricky_macros.h");
    let result = bic::HeaderConfig::new().entry_header(&header).process().unwrap();

    let api_level = result
        .package
        .macros
        .iter()
        .find(|macro_binding| macro_binding.name == "API_LEVEL")
        .unwrap();
    assert_eq!(api_level.value, Some(MacroValue::Integer(7)));

    let config_macro = result
        .package
        .effective_macro_environment
        .iter()
        .find(|entry| entry.macro_name == "HAVE_WIDGETS")
        .unwrap();
    assert_eq!(config_macro.value, Some(MacroValue::Integer(1)));

    let abi_macro = result
        .package
        .effective_macro_environment
        .iter()
        .find(|entry| entry.macro_name == "WIDGET_PACK")
        .unwrap();
    assert_eq!(abi_macro.category, bic::MacroCategory::AbiAffecting);

    let function_like = result
        .package
        .macros
        .iter()
        .find(|macro_binding| macro_binding.name == "DECLARE_WIDGET")
        .unwrap();
    assert!(function_like.is_unsupported_function_like());
}

#[test]
fn regression_linux_artifact_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../test/contracts/linux_elf_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Elf);
    assert_eq!(inventory.format, ArtifactFormat::ElfSharedLibrary);
    assert_eq!(inventory.dependency_edges, vec!["libc.so.6"]);
    assert!(inventory.has_symbol("demo_init"));
}

#[test]
fn regression_macos_artifact_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../test/contracts/macos_macho_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.format, ArtifactFormat::MachODylib);
    assert_eq!(inventory.symbols[0].raw_name.as_deref(), Some("_demo_init"));
}

#[test]
fn regression_windows_artifact_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../test/contracts/windows_coff_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::CoffObject);
    assert!(inventory.capabilities.exports_symbols);
    assert_eq!(inventory.symbols[0].raw_name.as_deref(), Some("_demo_init@4"));
}

#[test]
fn regression_windows_import_library_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../test/contracts/windows_import_library_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::CoffImportLibrary);
    assert_eq!(inventory.kind, ArtifactKind::ImportLibrary);
    assert!(inventory.capabilities.imports_symbols);
    assert_eq!(inventory.symbols[0].direction, bic::SymbolDirection::Imported);
}
