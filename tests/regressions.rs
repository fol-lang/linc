mod common;
use linc::ir::{
    AbiConfidence, BindingItem, BindingPackage, BindingType, CallingConvention, FunctionBinding,
    LinkInput, LinkLibrary, LinkLibraryKind, LinkRequirementSource, MacroCategory, MacroValue,
    ParameterBinding,
};
use linc::symbols::{ArtifactCapabilities, ArtifactFormat, ArtifactKind, ArtifactPlatform};
use linc::{
    resolve_link_plan_for_target, resolve_link_plan_with_inventories, validate, AbiProbeReport,
    DiagnosticKind, SymbolBinding, SymbolDirection, SymbolEntry, SymbolInventory, SymbolVisibility,
    ValidationReport,
};
use std::path::PathBuf;

#[test]
fn regression_old_json_with_empty_nested_objects_stays_consumable() {
    let pkg: BindingPackage = serde_json::from_str(include_str!(
        "../tests/contracts/v1_empty_nested_objects.json"
    ))
    .unwrap();
    assert_eq!(pkg.schema_version, linc::SCHEMA_VERSION);
    assert!(pkg.inputs.entry_headers.is_empty());
    assert!(pkg.link.ordered_inputs.is_empty());
}

#[test]
fn regression_target_filtered_link_plan_keeps_transitive_edges() {
    let mut package = BindingPackage::new();
    package.link.platform_constraints.push("linux".into());
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
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
        "../tests/contracts/decorated_symbol_inventory_fixture.json"
    ))
    .unwrap();

    let report = validate(&package, &inventory);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].status, linc::MatchStatus::Matched);
}

#[test]
fn regression_extended_contract_fixture_keeps_macro_value() {
    let pkg: BindingPackage = serde_json::from_str(include_str!(
        "../tests/contracts/fol_extended_contract.json"
    ))
    .unwrap();
    assert_eq!(pkg.macros[0].value, Some(MacroValue::Integer(3)));
}

#[test]
fn regression_probe_record_fixture_keeps_record_and_enum_metadata() {
    let report: AbiProbeReport = serde_json::from_str(include_str!(
        "../tests/contracts/probe_record_contract_snapshot.json"
    ))
    .unwrap();
    assert_eq!(report.subjects.len(), 2);
    assert_eq!(report.subjects[0].layout.name, "struct widget");
    assert_eq!(report.subjects[1].enum_underlying_size, Some(4));
}

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn regression_probe_diagnostics_distinguish_unavailable_and_operational_failures() {
    eprintln!("RUN: host C compiler probe-diagnostics regression evidence");
    let temp_root = std::env::temp_dir().join(format!(
        "bic_regression_probe_split_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_root).unwrap();
    let header = temp_root.join("probe_split.h");
    std::fs::write(
        &header,
        "typedef struct opaque_widget opaque_widget;\n\
         extern int opaque_use(opaque_widget *widget);\n\
         extern int concrete_use(int value);\n",
    )
    .unwrap();

    let unavailable = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .probe_type_layout("struct opaque_widget"),
    )
    .unwrap();
    assert_eq!(unavailable.package.probe_unavailable_count(), 1);
    assert_eq!(unavailable.package.probe_failure_count(), 0);
    assert!(unavailable
        .package
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::ProbeUnavailable));

    let failed = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .probe_type_layout("struct invalid["),
    )
    .unwrap();
    assert_eq!(failed.package.probe_unavailable_count(), 0);
    assert_eq!(failed.package.probe_failure_count(), 1);
    assert!(failed
        .package
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::ProbeFailed));

    std::fs::remove_file(&header).ok();
    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn regression_duplicate_provider_report_fixture_stays_consumable() {
    let report: ValidationReport = serde_json::from_str(include_str!(
        "../tests/contracts/validation_duplicate_provider_report.json"
    ))
    .unwrap();
    assert_eq!(report.summary.duplicate_providers, 1);
    assert_eq!(report.duplicate_providers().len(), 1);
    assert_eq!(
        report.entries[0].evidence.evidence_kind,
        linc::EvidenceKind::DuplicateVisibleProviders
    );
}

#[test]
fn regression_function_abi_questionable_fixture_stays_consumable() {
    let report: ValidationReport = serde_json::from_str(include_str!(
        "../tests/contracts/function_abi_questionable_report.json"
    ))
    .unwrap();
    assert_eq!(report.summary.abi_shape_mismatches, 1);
    assert_eq!(
        report.matches[0].status,
        linc::MatchStatus::AbiShapeMismatch
    );
    let routine = report.entries[0].evidence.routine_abi.as_ref().unwrap();
    assert_eq!(
        routine.evidence_kind,
        Some(linc::RoutineAbiEvidenceKind::Mismatch)
    );
    assert_eq!(
        routine.confidence,
        Some(linc::RoutineAbiConfidence::Mismatch)
    );
    assert_eq!(routine.expected_parameter_sizes, vec![Some(8), Some(4)]);
    assert_eq!(routine.observed_parameter_sizes, vec![Some(8), Some(8)]);
}

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn regression_tricky_layout_fixture_stays_consumable() {
    eprintln!("RUN: host C compiler tricky-layout regression evidence");
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tricky_layouts.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .probe_type_layout("struct packed_flags")
            .probe_type_layout("enum widget_mode"),
    )
    .unwrap();

    let alias = result.package.find_type_alias("my_size_ptr").unwrap();
    let resolution = alias.canonical_resolution.as_ref().unwrap();
    assert_eq!(resolution.alias_chain, vec!["my_size_t", "size_t"]);

    let record = result.package.find_record("packed_flags").unwrap();
    assert_eq!(
        record.abi_confidence,
        Some(AbiConfidence::PartialBitfieldLayout)
    );
    let fields = record.fields.as_ref().unwrap();
    assert_eq!(fields[0].bit_width, Some(3));
    assert_eq!(fields[1].bit_width, Some(5));

    let widget_mode = result.package.find_enum("widget_mode").unwrap();
    assert_eq!(
        widget_mode.abi_confidence,
        Some(AbiConfidence::RepresentationProbed)
    );
}

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn regression_typedef_layout_fixture_validates_record_and_enum_aliases() {
    eprintln!("RUN: host C compiler typedef-layout regression evidence");
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typedef_layout_bridge.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .probe_type_layout("widget_t")
            .probe_type_layout("mode_t"),
    )
    .unwrap();

    let inventory = SymbolInventory {
        artifact_path: "typedefs.o".into(),
        format: ArtifactFormat::ElfObject,
        platform: ArtifactPlatform::Elf,
        kind: ArtifactKind::Object,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: false,
        },
        dependency_edges: Vec::new(),
        symbols: vec![
            SymbolEntry {
                name: "widget_global".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                function_abi: None,
                visibility: SymbolVisibility::Default,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(16),
                section: Some(".data".into()),
                archive_member: None,
            },
            SymbolEntry {
                name: "current_mode".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                function_abi: None,
                visibility: SymbolVisibility::Default,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(4),
                section: Some(".data".into()),
                archive_member: None,
            },
        ],
    };

    let report = validate(&result.package, &inventory);
    assert_eq!(report.matches.len(), 2);
    assert!(report
        .matches
        .iter()
        .all(|entry| entry.status == linc::MatchStatus::Matched));
    assert_eq!(report.layout_backed_entries().len(), 2);
}

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn regression_packed_bitfield_fixture_preserves_partial_layout_signal() {
    eprintln!("RUN: host C compiler packed-bitfield regression evidence");
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packed_bitfield_extreme.h");
    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .entry_header(&header)
            .probe_type_layout("struct packed_registers")
            .probe_type_layout("packed_registers_t"),
    )
    .unwrap();

    let record = result.package.find_record("packed_registers").unwrap();
    assert_eq!(
        record.abi_confidence,
        Some(AbiConfidence::PartialBitfieldLayout)
    );
    let representation = record.representation.as_ref().unwrap();
    assert!(representation.size.is_some());
    let fields = record.fields.as_ref().unwrap();
    assert_eq!(fields[0].bit_width, Some(1));
    assert_eq!(fields[1].bit_width, Some(3));
    assert_eq!(fields[2].bit_width, Some(1));
    assert_eq!(fields[3].bit_width, Some(3));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct packed_registers"));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "packed_registers_t"));
}

#[test]
fn regression_link_plan_and_validation_agree_on_resolved_and_unresolved_providers() {
    let mut resolved_package = BindingPackage::new();
    resolved_package
        .items
        .push(BindingItem::Function(FunctionBinding {
            name: "zlibVersion".into(),
            calling_convention: CallingConvention::C,
            parameters: Vec::new(),
            return_type: BindingType::ptr(BindingType::Char),
            variadic: false,
            source_offset: None,
        }));
    resolved_package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "z".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let resolved_inventory = SymbolInventory {
        artifact_path: "/usr/lib/libz.so".into(),
        format: ArtifactFormat::ElfSharedLibrary,
        platform: ArtifactPlatform::Elf,
        kind: ArtifactKind::SharedLibrary,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: true,
        },
        dependency_edges: vec!["libc.so.6".into()],
        symbols: vec![SymbolEntry {
            name: "zlibVersion".into(),
            raw_name: Some("zlibVersion".into()),
            version: Some("ZLIB_1.2.0".into()),
            direction: SymbolDirection::Exported,
            reexported_via: Vec::new(),
            alias_of: None,
            function_abi: None,
            visibility: SymbolVisibility::Default,
            is_function: true,
            binding: SymbolBinding::Global,
            size: None,
            section: Some(".text".into()),
            archive_member: None,
        }],
    };

    let resolved_plan = resolve_link_plan_with_inventories(
        &resolved_package,
        std::slice::from_ref(&resolved_inventory),
    );
    let resolved_report = validate(&resolved_package, &resolved_inventory);
    assert_eq!(resolved_plan.requirements.len(), 1);
    assert_eq!(
        resolved_plan.requirements[0].resolution,
        linc::RequirementResolution::Resolved
    );
    assert_eq!(resolved_report.resolved_provider_entries().len(), 1);
    assert_eq!(resolved_report.unresolved_provider_entries().len(), 0);

    let mut unresolved_package = BindingPackage::new();
    unresolved_package
        .items
        .push(BindingItem::Function(FunctionBinding {
            name: "missing_symbol".into(),
            calling_convention: CallingConvention::C,
            parameters: Vec::new(),
            return_type: BindingType::Void,
            variadic: false,
            source_offset: None,
        }));
    unresolved_package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "missing".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let unresolved_inventory = SymbolInventory {
        artifact_path: "/tmp/other.so".into(),
        format: ArtifactFormat::ElfSharedLibrary,
        platform: ArtifactPlatform::Elf,
        kind: ArtifactKind::SharedLibrary,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: true,
        },
        dependency_edges: vec!["libdep.so".into()],
        symbols: Vec::new(),
    };

    let unresolved_plan = resolve_link_plan_with_inventories(
        &unresolved_package,
        std::slice::from_ref(&unresolved_inventory),
    );
    let unresolved_report = validate(&unresolved_package, &unresolved_inventory);
    assert_eq!(unresolved_plan.requirements.len(), 1);
    assert_eq!(
        unresolved_plan.requirements[0].resolution,
        linc::RequirementResolution::Unresolved
    );
    assert_eq!(unresolved_report.resolved_provider_entries().len(), 0);
    assert_eq!(unresolved_report.unresolved_provider_entries().len(), 1);
    assert_eq!(
        unresolved_report.entries[0].status,
        linc::MatchStatus::UnresolvedDeclaredLinkInputs
    );
}

#[test]
fn regression_macos_text_stub_provider_resolves_after_name_refinement() {
    let mut package = BindingPackage::new();
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "System".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

    let inventories = vec![SymbolInventory {
        artifact_path: "/usr/lib/libSystem.tbd".into(),
        format: ArtifactFormat::MachODylib,
        platform: ArtifactPlatform::MachO,
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
    assert_eq!(
        plan.requirements[0].resolution,
        linc::RequirementResolution::Resolved
    );
    assert_eq!(plan.requirements[0].providers.len(), 1);
    assert_eq!(
        plan.requirements[0].providers[0].artifact_path,
        "/usr/lib/libSystem.tbd"
    );
}

#[test]
#[ignore = "system prerequisite: host C preprocessor"]
fn regression_tricky_macro_fixture_stays_consumable() {
    eprintln!("RUN: host C preprocessor tricky-macro regression evidence");
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tricky_macros.h");
    let result =
        common::process(&linc::raw_headers::HeaderConfig::new().entry_header(&header)).unwrap();

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
    assert_eq!(abi_macro.category, MacroCategory::AbiAffecting);

    let function_like = result
        .package
        .macros
        .iter()
        .find(|macro_binding| macro_binding.name == "DECLARE_WIDGET")
        .unwrap();
    assert!(function_like.is_unsupported_function_like());
}

#[test]
#[ignore = "system prerequisite: host C preprocessor"]
fn regression_macro_public_api_fixture_preserves_configuration_and_abi_macros() {
    eprintln!("RUN: host C preprocessor macro-public-API regression evidence");
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/macro_public_api.h");
    let result =
        common::process(&linc::raw_headers::HeaderConfig::new().entry_header(&header)).unwrap();

    let api_level = result
        .package
        .macros
        .iter()
        .find(|entry| entry.name == "WIDGET_API_LEVEL")
        .unwrap();
    assert_eq!(api_level.value, Some(MacroValue::Integer(12)));
    assert_eq!(api_level.category, MacroCategory::BindableConstant);

    let fast_path = result
        .package
        .effective_macro_environment
        .iter()
        .find(|entry| entry.macro_name == "WIDGET_ENABLE_FAST_PATH")
        .unwrap();
    assert_eq!(fast_path.category, MacroCategory::ConfigurationFlag);

    let abi_macro = result
        .package
        .macros
        .iter()
        .find(|entry| entry.name == "WIDGET_PACKED")
        .unwrap();
    assert_eq!(abi_macro.category, MacroCategory::AbiAffecting);

    let function_like = result
        .package
        .macros
        .iter()
        .find(|entry| entry.name == "WIDGET_DECLARE_HANDLE")
        .unwrap();
    assert!(function_like.is_unsupported_function_like());
}

#[test]
fn regression_linux_artifact_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/linux_elf_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Elf);
    assert_eq!(inventory.format, ArtifactFormat::ElfSharedLibrary);
    assert_eq!(inventory.dependency_edges, vec!["libc.so.6"]);
    assert!(inventory.has_symbol("demo_init"));
}

#[test]
fn regression_linux_elf_mixed_fixture_preserves_versions_and_imports() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/linux_elf_mixed_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Elf);
    assert_eq!(inventory.format, ArtifactFormat::ElfSharedLibrary);
    assert_eq!(inventory.dependency_edges, vec!["libc.so.6", "libm.so.6"]);
    assert_eq!(inventory.symbols.len(), 3);
    assert_eq!(inventory.symbols[0].version.as_deref(), Some("WIDGET_1.0"));
    assert_eq!(inventory.symbols[1].direction, SymbolDirection::Imported);
    assert_eq!(inventory.symbols[1].reexported_via, vec!["libm.so.6"]);
    assert_eq!(inventory.symbols[2].size, Some(4));
}

#[test]
fn regression_elf_alias_fixture_preserves_aliases_without_duplicate_provider_confusion() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/elf_alias_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.symbols.len(), 2);
    assert_eq!(inventory.symbols[1].alias_of.as_deref(), Some("alias_init"));

    let package = BindingPackage {
        items: vec![
            BindingItem::Function(FunctionBinding {
                name: "alias_init".into(),
                calling_convention: CallingConvention::C,
                parameters: Vec::new(),
                return_type: BindingType::Void,
                variadic: false,
                source_offset: None,
            }),
            BindingItem::Function(FunctionBinding {
                name: "alias_bootstrap".into(),
                calling_convention: CallingConvention::C,
                parameters: Vec::new(),
                return_type: BindingType::Void,
                variadic: false,
                source_offset: None,
            }),
        ],
        ..BindingPackage::new()
    };
    let report = validate(&package, &inventory);
    assert_eq!(report.matches.len(), 2);
    assert!(report
        .matches
        .iter()
        .all(|entry| entry.status == linc::MatchStatus::Matched));
    assert_eq!(report.duplicate_providers().len(), 0);
}

#[test]
fn regression_macos_artifact_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_macho_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.format, ArtifactFormat::MachODylib);
    assert_eq!(inventory.symbols[0].raw_name.as_deref(), Some("_demo_init"));
}

#[test]
fn regression_macos_dylib_mixed_fixture_preserves_imported_and_exported_symbols() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/macos_macho_dylib_mixed_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::MachO);
    assert_eq!(inventory.format, ArtifactFormat::MachODylib);
    assert_eq!(
        inventory.dependency_edges,
        vec!["/usr/lib/libSystem.B.dylib"]
    );
    assert_eq!(
        inventory.symbols[0].raw_name.as_deref(),
        Some("_widget_init")
    );
    assert_eq!(inventory.symbols[1].direction, SymbolDirection::Imported);
    assert_eq!(
        inventory.symbols[1].reexported_via,
        vec!["/usr/lib/libSystem.B.dylib"]
    );
}

#[test]
fn regression_windows_artifact_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_coff_inventory_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::CoffObject);
    assert!(inventory.capabilities.exports_symbols);
    assert_eq!(
        inventory.symbols[0].raw_name.as_deref(),
        Some("_demo_init@4")
    );
}

#[test]
fn regression_windows_import_library_fixture_stays_consumable() {
    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/windows_import_library_fixture.json"
    ))
    .unwrap();
    assert_eq!(inventory.platform, ArtifactPlatform::Windows);
    assert_eq!(inventory.format, ArtifactFormat::CoffImportLibrary);
    assert_eq!(inventory.kind, ArtifactKind::ImportLibrary);
    assert!(inventory.capabilities.imports_symbols);
    assert_eq!(inventory.symbols[0].direction, SymbolDirection::Imported);
}
