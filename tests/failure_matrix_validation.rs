use linc::ir::{BindingItem, BindingPackage, BindingType, VariableBinding};
use linc::symbols::{
    ArtifactCapabilities, ArtifactFormat, ArtifactKind, ArtifactPlatform, SymbolBinding,
    SymbolDirection, SymbolEntry, SymbolInventory, SymbolVisibility,
};
use linc::{validate, EvidenceKind, MatchStatus, ValidationReport};

#[test]
fn failure_matrix_validation_abi_questionable_fixture_stays_explicit() {
    let report: ValidationReport = serde_json::from_str(include_str!(
        "../tests/contracts/function_abi_questionable_report.json"
    ))
    .unwrap();

    assert_eq!(report.summary.abi_shape_mismatches, 1);
    assert_eq!(report.matches[0].status, MatchStatus::AbiShapeMismatch);
    assert_eq!(
        report.matches[0].evidence_kind,
        EvidenceKind::AbiShapeMismatch
    );
}

#[test]
fn failure_matrix_validation_hidden_and_kind_mismatches_do_not_look_matched() {
    let package = BindingPackage {
        items: vec![
            BindingItem::Variable(VariableBinding {
                name: "hidden_data".into(),
                ty: BindingType::Int,
                source_offset: None,
            }),
            BindingItem::Variable(VariableBinding {
                name: "wrong_kind_data".into(),
                ty: BindingType::Int,
                source_offset: None,
            }),
        ],
        ..BindingPackage::new()
    };

    let inventory = SymbolInventory {
        artifact_path: "libdemo.a".into(),
        format: ArtifactFormat::ElfStaticLibrary,
        platform: ArtifactPlatform::Elf,
        kind: ArtifactKind::StaticLibrary,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: false,
        },
        dependency_edges: Vec::new(),
        symbols: vec![
            SymbolEntry {
                name: "hidden_data".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                function_abi: None,
                visibility: SymbolVisibility::Hidden,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(4),
                section: None,
                archive_member: None,
            },
            SymbolEntry {
                name: "wrong_kind_data".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                function_abi: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
            },
        ],
    };

    let report = validate(&package, &inventory);
    assert_eq!(report.matches.len(), 2);
    assert!(report
        .matches
        .iter()
        .any(|m| m.name == "hidden_data" && m.status == MatchStatus::Hidden));
    assert!(report
        .matches
        .iter()
        .any(|m| m.name == "wrong_kind_data" && m.status == MatchStatus::NotAVariable));
}
