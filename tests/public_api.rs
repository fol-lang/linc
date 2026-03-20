use linc::{
    AbiConfidence, AbiProbeReport, AliasResolution, LincError, BindingItem, BindingPackage, BindingType,
    CallingConvention, EvidenceKind, FunctionBinding, HeaderConfig, LinkResolutionMode,
    MacroBinding, MacroCategory, MacroForm, MacroKind, MacroValue, MatchConfidence,
    ParameterBinding, ProbeConfidence, ProbeSubjectKind, ProbeSubjectReport, ProbedFieldLayout,
    RecordCompleteness, TypeAliasBinding, TypeLayout, TypeQualifiers, ValidationDeclaration, ValidationEntry,
    ValidationEvidence, ValidationPhase, ValidationPhaseReport, ValidationSummary, RoutineAbiConfidence,
    RoutineAbiEvidence, RoutineAbiEvidenceKind,
    EnumRepresentation, FieldLayout,
    RecordRepresentation, probe_type_layouts,
    // Intake types (Phase 1)
    SourcePackage, SourceDeclaration, SourceFunction, SourceType, SourceRecord,
    SourceEnum, SourceTypeAlias, SourceVariable, from_source_package,
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
        canonical_resolution: None,
        abi_confidence: None,
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
    assert_eq!(package.probe_unavailable_count(), 0);
    assert_eq!(package.probe_failure_count(), 0);
    assert!(!package.has_probe_unavailable_diagnostics());
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

    assert!(matches!(err, LincError::InvalidConfig { .. }));
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

    assert!(matches!(err, LincError::InvalidConfig { .. }));
}

#[test]
fn abi_probe_report_root_types_roundtrip() {
    let report = AbiProbeReport {
        target: linc::BindingTarget {
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
fn record_representation_root_type_roundtrip() {
    let representation = RecordRepresentation {
        size: Some(16),
        align: Some(8),
        completeness: Some("Complete".into()),
    };
    let json = serde_json::to_string(&representation).unwrap();
    let decoded: RecordRepresentation = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, representation);
}

#[test]
fn abi_confidence_root_type_roundtrip() {
    let json = serde_json::to_string(&AbiConfidence::FieldOffsetsProbed).unwrap();
    let decoded: AbiConfidence = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, AbiConfidence::FieldOffsetsProbed);
}

#[test]
fn routine_abi_evidence_root_type_roundtrip() {
    let evidence = RoutineAbiEvidence {
        evidence_kind: Some(RoutineAbiEvidenceKind::FullyShaped),
        confidence: Some(RoutineAbiConfidence::Strong),
        expected_parameter_count: Some(2),
        observed_parameter_count: Some(2),
        expected_return_size: Some(4),
        observed_return_size: Some(4),
        expected_parameter_sizes: vec![Some(4), Some(4)],
        observed_parameter_sizes: vec![Some(4), Some(4)],
    };
    let json = serde_json::to_string(&evidence).unwrap();
    let decoded: RoutineAbiEvidence = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, evidence);
}

#[test]
fn alias_resolution_root_type_roundtrip() {
    let resolution = AliasResolution {
        alias_chain: vec!["size_t".into()],
        terminal_target: BindingType::ULong,
    };
    let json = serde_json::to_string(&resolution).unwrap();
    let decoded: AliasResolution = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, resolution);
}

#[test]
fn type_qualifiers_root_type_roundtrip() {
    let qualifiers = TypeQualifiers {
        is_const: true,
        is_volatile: true,
        is_restrict: false,
        is_atomic: false,
    };
    let json = serde_json::to_string(&qualifiers).unwrap();
    let decoded: TypeQualifiers = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, qualifiers);
}

#[test]
fn calling_convention_root_type_roundtrip() {
    let json = serde_json::to_string(&linc::CallingConvention::Stdcall).unwrap();
    let decoded: linc::CallingConvention = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, linc::CallingConvention::Stdcall);
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
            item_kind: linc::ItemKind::Function,
        },
        status: linc::MatchStatus::Matched,
        evidence: ValidationEvidence {
            provider_artifacts: vec!["libc.so".into()],
            raw_symbol_names: vec!["_malloc".into()],
            visibility: Some(linc::SymbolVisibility::Default),
            confidence: MatchConfidence::High,
            evidence_kind: EvidenceKind::ExactExported,
            abi_shape: None,
            routine_abi: None,
        },
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: ValidationEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, entry);
    assert!(!decoded.has_layout_backed_confidence());
}

#[test]
fn abi_shape_evidence_root_type_roundtrip() {
    let evidence = linc::AbiShapeEvidence {
        expected_size: Some(4),
        observed_size: Some(8),
    };
    let json = serde_json::to_string(&evidence).unwrap();
    let decoded: linc::AbiShapeEvidence = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, evidence);
}

#[test]
fn validation_evidence_reports_layout_backed_confidence() {
    let entry = ValidationEntry {
        declaration: ValidationDeclaration {
            name: "errno".into(),
            item_kind: linc::ItemKind::Variable,
        },
        status: linc::MatchStatus::Matched,
        evidence: ValidationEvidence {
            provider_artifacts: vec!["libc.so".into()],
            raw_symbol_names: vec!["errno".into()],
            visibility: Some(linc::SymbolVisibility::Default),
            confidence: MatchConfidence::High,
            evidence_kind: EvidenceKind::AbiShapeVerified,
            abi_shape: Some(linc::AbiShapeEvidence {
                expected_size: Some(4),
                observed_size: Some(4),
            }),
            routine_abi: None,
        },
    };

    assert!(entry.evidence.has_layout_backed_confidence());
    assert!(entry.has_layout_backed_confidence());
    assert!(entry.has_resolved_provider_state());
    assert!(!entry.has_unresolved_provider_state());
    assert!(!entry.has_ambiguous_provider_state());
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
fn intake_types_reachable_from_root() {
    let mut src = SourcePackage::default();
    src.source_path = Some("api.h".to_string());
    src.declarations.push(SourceDeclaration::Function(SourceFunction {
        name: "init".into(),
        parameters: vec![],
        return_type: SourceType::Int,
        variadic: false,
        source_offset: None,
    }));
    src.declarations.push(SourceDeclaration::Record(SourceRecord {
        name: Some("config".into()),
        fields: Some(vec![]),
        is_union: false,
        source_offset: None,
    }));
    src.declarations.push(SourceDeclaration::Enum(SourceEnum {
        name: Some("mode".into()),
        variants: vec![],
        source_offset: None,
    }));
    src.declarations.push(SourceDeclaration::TypeAlias(SourceTypeAlias {
        name: "size_t".into(),
        target: SourceType::ULong,
        source_offset: None,
    }));
    src.declarations.push(SourceDeclaration::Variable(SourceVariable {
        name: "errno".into(),
        ty: SourceType::Int,
        source_offset: None,
    }));

    let pkg = from_source_package(&src);

    assert_eq!(pkg.item_count(), 5);
    assert_eq!(pkg.function_count(), 1);
    assert!(pkg.find_function("init").is_some());
    assert!(pkg.find_record("config").is_some());
    assert!(pkg.find_type_alias("size_t").is_some());
}

#[test]
fn linc_error_alias_is_reachable() {
    let err: Result<(), LincError> = Err(LincError::NoProbeTypes);
    assert!(err.is_err());
}

#[test]
fn validation_summary_root_type_roundtrip() {
    let summary = ValidationSummary {
        total: 3,
        matched: 1,
        abi_shape_mismatches: 0,
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
