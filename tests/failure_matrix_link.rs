use linc::ir::{
    BindingItem, BindingPackage, BindingType, CallingConvention, FunctionBinding, LinkInput,
    LinkLibrary, LinkLibraryKind, LinkRequirementSource,
};
use linc::symbols::{
    ArtifactCapabilities, ArtifactFormat, ArtifactKind, ArtifactPlatform, SymbolInventory,
};
use linc::{
    resolve_link_plan_with_inventories, MatchStatus, RequirementResolution, ValidationReport,
};

#[test]
fn failure_matrix_link_unresolved_provider_is_explicit_in_link_plan() {
    let mut package = BindingPackage::new();
    package.items.push(BindingItem::Function(FunctionBinding {
        name: "demo_init".into(),
        calling_convention: CallingConvention::C,
        parameters: vec![],
        return_type: BindingType::Int,
        variadic: false,
        source_offset: None,
    }));
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let inventories = vec![SymbolInventory {
        artifact_path: "/tmp/libother.so".into(),
        format: ArtifactFormat::ElfSharedLibrary,
        platform: ArtifactPlatform::Elf,
        kind: ArtifactKind::SharedLibrary,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: true,
        },
        dependency_edges: Vec::new(),
        symbols: Vec::new(),
    }];

    let plan = resolve_link_plan_with_inventories(&package, &inventories);
    assert_eq!(plan.requirements.len(), 1);
    assert!(matches!(
        plan.requirements[0].resolution,
        RequirementResolution::Unresolved
    ));
}

#[test]
fn failure_matrix_link_duplicate_provider_fixture_stays_grouped() {
    let report: ValidationReport = serde_json::from_str(include_str!(
        "../tests/contracts/validation_duplicate_provider_report.json"
    ))
    .unwrap();

    assert_eq!(report.summary.duplicate_providers, 1);
    assert_eq!(report.duplicate_providers().len(), 1);
    assert_eq!(report.matches[0].status, MatchStatus::DuplicateProviders);
}
