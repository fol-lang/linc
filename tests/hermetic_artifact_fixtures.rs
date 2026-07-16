use linc::ir::{
    BindingLinkSurface, BindingPackage, LinkFramework, LinkInput, LinkLibrary, LinkLibraryKind,
    LinkRequirementSource,
};
use linc::symbols::{ArtifactFormat, ArtifactKind, ArtifactPlatform, SymbolInventory};
use linc::{resolve_link_plan_with_inventories, ProviderMatchKind, RequirementResolution};

#[test]
fn hermetic_artifact_fixture_elf_static_archive_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/linux_elf_static_archive_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::Elf);
    assert_eq!(inventory.format, ArtifactFormat::ElfStaticLibrary);
    assert_eq!(inventory.kind, ArtifactKind::StaticLibrary);
    assert!(inventory.capabilities.exports_symbols);
    assert_eq!(
        inventory.symbols[0].archive_member.as_deref(),
        Some("widget_init.o")
    );

    let mut package = BindingPackage::new();
    package.link = BindingLinkSurface {
        ordered_inputs: vec![LinkInput::Library(LinkLibrary {
            name: "widget".into(),
            kind: LinkLibraryKind::Static,
            source: LinkRequirementSource::Declared,
        })],
        ..BindingLinkSurface::default()
    };

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(
        plan.requirements[0].resolution,
        RequirementResolution::Resolved
    );
    assert_eq!(
        plan.requirements[0].providers[0].match_kind,
        ProviderMatchKind::LibraryName
    );
}

#[test]
fn hermetic_artifact_fixture_macho_framework_binary_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_framework_binary_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.format, ArtifactFormat::MachODylib);
    assert_eq!(inventory.kind, ArtifactKind::SharedLibrary);
    assert_eq!(
        inventory.symbols[0].raw_name.as_deref(),
        Some("_SecKeyCreateRandomKey")
    );

    let mut package = BindingPackage::new();
    package.link = BindingLinkSurface {
        ordered_inputs: vec![LinkInput::Framework(LinkFramework {
            name: "Security".into(),
            source: LinkRequirementSource::Declared,
        })],
        ..BindingLinkSurface::default()
    };

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(
        plan.requirements[0].resolution,
        RequirementResolution::Resolved
    );
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
fn hermetic_artifact_fixture_windows_pe_dylib_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_pe_dynamic_library_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::PeDynamicLibrary);
    assert_eq!(inventory.kind, ArtifactKind::SharedLibrary);
    assert!(inventory.capabilities.imports_symbols);

    let mut package = BindingPackage::new();
    package.link = BindingLinkSurface {
        ordered_inputs: vec![LinkInput::Library(LinkLibrary {
            name: "bcrypt".into(),
            kind: LinkLibraryKind::Dynamic,
            source: LinkRequirementSource::Declared,
        })],
        ..BindingLinkSurface::default()
    };

    let plan = resolve_link_plan_with_inventories(&package, &[inventory]);
    assert_eq!(
        plan.requirements[0].resolution,
        RequirementResolution::Resolved
    );
    assert_eq!(
        plan.requirements[0].providers[0].match_kind,
        ProviderMatchKind::LibraryName
    );
    assert_eq!(
        plan.transitive_dependencies,
        vec!["KERNEL32.dll".to_string()]
    );
}
