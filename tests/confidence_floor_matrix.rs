use linc::symbols::{
    ArtifactFormat, ArtifactKind, ArtifactPlatform, SymbolDirection, SymbolInventory,
};

#[test]
fn confidence_floor_matrix_elf_fixture_stays_structured() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/linux_elf_inventory_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::Elf);
    assert_eq!(inventory.format, ArtifactFormat::ElfSharedLibrary);
    assert_eq!(inventory.kind, ArtifactKind::SharedLibrary);
    assert!(inventory.capabilities.exports_symbols);
    assert!(inventory.capabilities.imports_symbols);
    assert!(inventory
        .symbols
        .iter()
        .any(|symbol| symbol.direction == SymbolDirection::Exported));
}

#[test]
fn confidence_floor_matrix_macho_fixture_stays_structured() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_macho_inventory_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.format, ArtifactFormat::MachODylib);
    assert_eq!(inventory.kind, ArtifactKind::SharedLibrary);
    assert!(inventory.capabilities.exports_symbols);
    assert!(inventory.capabilities.imports_symbols);
    assert_eq!(inventory.symbols[0].raw_name.as_deref(), Some("_demo_init"));
}

#[test]
fn confidence_floor_matrix_windows_object_fixture_stays_structured() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_coff_inventory_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::CoffObject);
    assert_eq!(inventory.kind, ArtifactKind::Object);
    assert!(inventory.capabilities.exports_symbols);
    assert!(!inventory.capabilities.imports_symbols);
    assert_eq!(
        inventory.symbols[0].raw_name.as_deref(),
        Some("_demo_init@4")
    );
}

#[test]
fn confidence_floor_matrix_windows_import_fixture_stays_structured() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_import_library_fixture.json"
    ))
    .unwrap();

    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::CoffImportLibrary);
    assert_eq!(inventory.kind, ArtifactKind::ImportLibrary);
    assert!(!inventory.capabilities.exports_symbols);
    assert!(inventory.capabilities.imports_symbols);
    assert_eq!(inventory.symbols[0].direction, SymbolDirection::Imported);
}
