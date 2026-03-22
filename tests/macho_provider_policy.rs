use linc::ir::{
    BindingLinkSurface, BindingPackage, LinkFramework, LinkInput, LinkLibrary, LinkLibraryKind,
    LinkRequirementSource,
};
use linc::symbols::{ArtifactFormat, ArtifactKind, ArtifactPlatform, SymbolInventory};
use linc::{resolve_link_plan_with_inventories, ProviderMatchKind, RequirementResolution};

#[test]
fn macho_provider_policy_matches_declared_framework_inputs() {
    let inventory = SymbolInventory {
        artifact_path:
            "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation".into(),
        format: ArtifactFormat::MachODylib,
        platform: ArtifactPlatform::MachO,
        kind: ArtifactKind::SharedLibrary,
        capabilities: Default::default(),
        dependency_edges: vec!["/usr/lib/libSystem.B.dylib".into()],
        symbols: Vec::new(),
    };

    let mut package = BindingPackage::new();
    package.link = BindingLinkSurface {
        ordered_inputs: vec![LinkInput::Framework(LinkFramework {
            name: "CoreFoundation".into(),
            source: LinkRequirementSource::Declared,
        })],
        ..BindingLinkSurface::default()
    };

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(plan.requirements[0].resolution, RequirementResolution::Resolved);
    assert_eq!(plan.requirements[0].providers.len(), 1);
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
fn macho_provider_policy_matches_dylib_library_inputs_and_preserves_dependency_edges() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_macho_dylib_mixed_fixture.json"
    ))
    .unwrap();

    let mut package = BindingPackage::new();
    package.link = BindingLinkSurface {
        ordered_inputs: vec![LinkInput::Library(LinkLibrary {
            name: "widget".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        })],
        ..BindingLinkSurface::default()
    };

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(plan.requirements.len(), 1);
    assert_eq!(plan.requirements[0].resolution, RequirementResolution::Resolved);
    assert_eq!(plan.requirements[0].providers.len(), 1);
    assert_eq!(
        plan.requirements[0].providers[0].match_kind,
        ProviderMatchKind::LibraryName
    );
    assert_eq!(
        plan.transitive_dependencies,
        vec!["/usr/lib/libSystem.B.dylib".to_string()]
    );
}
