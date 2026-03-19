use bic::{
    from_json, resolve_link_plan_for_target, validate, AbiProbeReport, BindingItem,
    BindingPackage, BindingType, CallingConvention, FunctionBinding, LinkInput, LinkLibrary,
    LinkLibraryKind, LinkRequirementSource, MacroValue, ParameterBinding, SymbolInventory,
    ValidationReport,
};
use bic::symbols::{ArtifactCapabilities, ArtifactFormat, ArtifactKind, ArtifactPlatform};

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
