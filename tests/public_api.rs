use bic::{
    AbiProbeReport, BicError, BindingItem, BindingPackage, BindingType, CallingConvention,
    EvidenceKind, FunctionBinding, HeaderConfig, LinkResolutionMode, MacroBinding, MacroCategory,
    MacroForm, MacroKind, MacroValue, MatchConfidence, ParameterBinding, ProbeConfidence,
    ProbeSubjectKind, ProbeSubjectReport, ProbedFieldLayout, RecordCompleteness, TypeAliasBinding,
    TypeLayout, ValidationDeclaration, ValidationEntry, ValidationEvidence, ValidationPhase,
    ValidationPhaseReport, ValidationSummary, EnumRepresentation, FieldLayout, probe_type_layouts,
};

#[test]
fn binding_package_public_helpers_are_available_from_root() {
    let mut package = BindingPackage::new();
    package.macros.push(MacroBinding {
        name: "API_LEVEL".into(),
        body: "7".into(),
        function_like: false,
        form: MacroForm::ObjectLike,
        kind: MacroKind::Integer,
        category: MacroCategory::BindableConstant,
        value: Some(MacroValue::Integer(7)),
    });
    package.items.push(BindingItem::TypeAlias(TypeAliasBinding {
        name: "size_t".into(),
        target: BindingType::ULong,
        source_offset: Some(1),
    }));
    package.items.push(BindingItem::Function(FunctionBinding {
        name: "malloc".into(),
        calling_convention: CallingConvention::C,
        parameters: vec![ParameterBinding {
            name: Some("size".into()),
            ty: BindingType::ULong,
        }],
        return_type: BindingType::ptr(BindingType::Void),
        variadic: false,
        source_offset: Some(2),
    }));

    assert_eq!(package.item_count(), 2);
    assert_eq!(package.function_count(), 1);
    assert_eq!(package.type_alias_count(), 1);
    assert_eq!(package.functions().next().map(|item| item.name.as_str()), Some("malloc"));
    assert_eq!(
        package.find_type_alias("size_t").map(|item| item.name.as_str()),
        Some("size_t")
    );
}

#[test]
fn header_config_validation_is_publicly_reachable() {
    let config = HeaderConfig::new()
        .entry_header("demo.h")
        .add_include_dir("include")
        .define_flag("DEBUG")
        .prefer_dynamic_linking();

    config.validate().unwrap();
    assert_eq!(config.linking().preferred_link_mode, LinkResolutionMode::PreferDynamic);

    let invalid = HeaderConfig::new().entry_header("");
    assert!(invalid.validate().is_err());
}

#[test]
fn process_rejects_invalid_config_before_execution() {
    let err = HeaderConfig::new()
        .entry_header("demo.h")
        .add_include_dir("")
        .process()
        .unwrap_err();

    assert!(matches!(err, BicError::InvalidConfig { .. }));
}

#[test]
fn probe_rejects_invalid_config_before_execution() {
    let err = probe_type_layouts(
        &HeaderConfig::new()
            .entry_header("demo.h")
            .add_include_dir(""),
        &["size_t"],
    )
    .unwrap_err();

    assert!(matches!(err, BicError::InvalidConfig { .. }));
}

#[test]
fn abi_probe_report_root_types_roundtrip() {
    let report = AbiProbeReport {
        target: bic::BindingTarget {
            target_triple: Some("x86_64-unknown-linux-gnu".into()),
            compiler_command: Some("clang".into()),
            compiler_version: Some("clang 18.0.0".into()),
            flavor: Some("clang-c11".into()),
        },
        compiler_command: "clang".into(),
        entry_headers: vec!["demo.h".into()],
        subjects: vec![ProbeSubjectReport {
            name: "size_t".into(),
            kind: ProbeSubjectKind::Type,
            confidence: ProbeConfidence::MeasuredLayout,
            record_completeness: None,
            enum_underlying_size: None,
            enum_is_signed: None,
            fields: Vec::new(),
            layout: TypeLayout {
                name: "size_t".into(),
                size: 8,
                align: 8,
            },
        }],
        layouts: vec![TypeLayout {
            name: "size_t".into(),
            size: 8,
            align: 8,
        }],
    };

    let json = serde_json::to_string(&report).unwrap();
    let decoded: AbiProbeReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, report);
}

#[test]
fn probe_subject_report_supports_record_completeness_metadata() {
    let subject = ProbeSubjectReport {
        name: "struct widget".into(),
        kind: ProbeSubjectKind::Record,
        confidence: ProbeConfidence::MeasuredLayout,
        record_completeness: Some(RecordCompleteness::Complete),
        enum_underlying_size: None,
        enum_is_signed: None,
        fields: vec![ProbedFieldLayout {
            name: "x".into(),
            offset_bytes: Some(0),
            bit_width: None,
        }],
        layout: TypeLayout {
            name: "struct widget".into(),
            size: 16,
            align: 8,
        },
    };

    let json = serde_json::to_string(&subject).unwrap();
    let decoded: ProbeSubjectReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, subject);
}

#[test]
fn probed_field_layout_supports_partial_bitfield_metadata() {
    let field = ProbedFieldLayout {
        name: "value".into(),
        offset_bytes: None,
        bit_width: Some(3),
    };
    let json = serde_json::to_string(&field).unwrap();
    let decoded: ProbedFieldLayout = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, field);
}

#[test]
fn field_layout_root_type_roundtrip() {
    let layout = FieldLayout {
        offset_bytes: Some(8),
    };
    let json = serde_json::to_string(&layout).unwrap();
    let decoded: FieldLayout = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, layout);
}

#[test]
fn enum_representation_root_type_roundtrip() {
    let representation = EnumRepresentation {
        underlying_size: Some(4),
        is_signed: Some(true),
    };
    let json = serde_json::to_string(&representation).unwrap();
    let decoded: EnumRepresentation = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, representation);
}

#[test]
fn probe_subject_report_supports_enum_representation_metadata() {
    let subject = ProbeSubjectReport {
        name: "enum mode".into(),
        kind: ProbeSubjectKind::Enum,
        confidence: ProbeConfidence::MeasuredLayout,
        record_completeness: None,
        enum_underlying_size: Some(4),
        enum_is_signed: Some(true),
        fields: Vec::new(),
        layout: TypeLayout {
            name: "enum mode".into(),
            size: 4,
            align: 4,
        },
    };

    let json = serde_json::to_string(&subject).unwrap();
    let decoded: ProbeSubjectReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.enum_underlying_size, Some(4));
    assert_eq!(decoded.enum_is_signed, Some(true));
    assert_eq!(decoded, subject);
}

#[test]
fn validation_phase_report_root_types_roundtrip() {
    let phase = ValidationPhaseReport {
        phase: ValidationPhase::ProviderDiscovery,
        completed: true,
    };
    let json = serde_json::to_string(&phase).unwrap();
    let decoded: ValidationPhaseReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, phase);
}

#[test]
fn validation_entry_root_types_roundtrip() {
    let entry = ValidationEntry {
        declaration: ValidationDeclaration {
            name: "malloc".into(),
            item_kind: bic::ItemKind::Function,
        },
        status: bic::MatchStatus::Matched,
        evidence: ValidationEvidence {
            provider_artifacts: vec!["libc.so".into()],
            raw_symbol_names: vec!["_malloc".into()],
            visibility: Some(bic::SymbolVisibility::Default),
            confidence: MatchConfidence::High,
            evidence_kind: EvidenceKind::ExactExported,
        },
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: ValidationEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, entry);
}

#[test]
fn match_confidence_root_type_roundtrip() {
    let json = serde_json::to_string(&MatchConfidence::Medium).unwrap();
    let decoded: MatchConfidence = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, MatchConfidence::Medium);
}

#[test]
fn evidence_kind_root_type_roundtrip() {
    let json = serde_json::to_string(&EvidenceKind::WeakExported).unwrap();
    let decoded: EvidenceKind = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, EvidenceKind::WeakExported);
}

#[test]
fn validation_summary_root_type_roundtrip() {
    let summary = ValidationSummary {
        total: 3,
        matched: 1,
        missing: 1,
        unresolved_declared_link_inputs: 0,
        hidden: 0,
        weak_matches: 1,
        duplicate_providers: 0,
        decoration_mismatches: 0,
        kind_mismatches: 0,
    };
    let json = serde_json::to_string(&summary).unwrap();
    let decoded: ValidationSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, summary);
}
