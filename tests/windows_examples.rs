use linc::ir::{
    BindingItem, BindingPackage, BindingType, CallingConvention, FunctionBinding, LinkInput,
    LinkLibrary, LinkLibraryKind, LinkRequirementSource, ParameterBinding,
};
use linc::symbols::{ArtifactPlatform, SymbolDirection, SymbolInventory};
use linc::{
    resolve_link_plan_with_inventories, validate, MatchStatus, ProviderMatchKind,
    RequirementResolution,
};

#[test]
fn windows_native_pe_fixture_resolves_declared_library_and_keeps_kernel_edges() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_pe_dynamic_library_fixture.json"
    ))
    .unwrap();

    let mut package = BindingPackage::new();
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "bcrypt".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(
        plan.requirements[0].resolution,
        RequirementResolution::Resolved
    );
    assert_eq!(plan.requirements[0].providers.len(), 1);
    assert_eq!(
        plan.requirements[0].providers[0].match_kind,
        ProviderMatchKind::LibraryName
    );
    assert_eq!(
        plan.requirements[0].providers[0].artifact_path,
        "C:/Windows/System32/bcrypt.dll"
    );
    assert_eq!(
        plan.transitive_dependencies,
        vec!["KERNEL32.dll".to_string()]
    );
}

#[test]
fn windows_native_import_fixture_resolves_declared_library_name() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_import_library_fixture.json"
    ))
    .unwrap();

    let mut package = BindingPackage::new();
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(
        plan.requirements[0].resolution,
        RequirementResolution::Resolved
    );
    assert_eq!(plan.requirements[0].providers[0].artifact_path, "demo.lib");
}

#[test]
fn windows_native_coff_fixture_keeps_export_validation_usable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_coff_inventory_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.symbols[0].direction, SymbolDirection::Exported);

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

    let report = validate(&package, &inventory);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].status, MatchStatus::Matched);
}
