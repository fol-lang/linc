use linc::ir::{
    BindingItem, BindingLinkSurface, BindingPackage, FunctionBinding, LinkFramework, LinkInput,
    LinkLibrary, LinkLibraryKind, LinkRequirementSource,
};
use linc::symbols::{ArtifactFormat, ArtifactPlatform, SymbolDirection, SymbolInventory};
use linc::{resolve_link_plan_with_inventories, ProviderMatchKind, RequirementResolution};

#[test]
fn macos_framework_fixture_resolves_declared_framework_and_keeps_system_dependency() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_framework_binary_fixture.json"
    ))
    .unwrap();

    let mut package = BindingPackage::new();
    package.link = BindingLinkSurface {
        ordered_inputs: vec![LinkInput::Framework(LinkFramework {
            name: "Security".into(),
            source: LinkRequirementSource::Declared,
        })],
        ..BindingLinkSurface::default()
    };

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(plan.requirements[0].resolution, RequirementResolution::Resolved);
    assert_eq!(
        plan.requirements[0].providers[0].match_kind,
        ProviderMatchKind::FrameworkName
    );
    assert_eq!(
        plan.transitive_dependencies,
        vec!["/usr/lib/libSystem.B.dylib".to_string()]
    );
}

#[test]
fn macos_dylib_fixture_resolves_declared_library_and_preserves_macho_identity() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_macho_dylib_mixed_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.format, ArtifactFormat::MachODylib);
    assert!(inventory
        .symbols
        .iter()
        .any(|symbol| symbol.direction == SymbolDirection::Imported));
    assert!(inventory
        .symbols
        .iter()
        .any(|symbol| symbol.direction == SymbolDirection::Exported));

    let mut package = BindingPackage::new();
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "widget".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(plan.requirements[0].resolution, RequirementResolution::Resolved);
    assert_eq!(
        plan.requirements[0].providers[0].match_kind,
        ProviderMatchKind::LibraryName
    );
    assert_eq!(
        plan.transitive_dependencies,
        vec!["/usr/lib/libSystem.B.dylib".to_string()]
    );
}

#[test]
fn macos_inventory_fixture_keeps_prefixed_export_names_normalized() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_macho_inventory_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.symbols[0].raw_name.as_deref(), Some("_demo_init"));
    assert_eq!(inventory.symbols[0].name, "demo_init");

    let package = BindingPackage {
        items: vec![BindingItem::Function(FunctionBinding {
            name: "demo_init".into(),
            calling_convention: linc::ir::CallingConvention::C,
            parameters: Vec::new(),
            return_type: linc::ir::BindingType::Int,
            variadic: false,
            source_offset: None,
        })],
        ..BindingPackage::new()
    };

    let report = linc::validate(&package, &inventory);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].status, linc::MatchStatus::Matched);
}
