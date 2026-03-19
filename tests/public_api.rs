use bic::{
    AbiProbeReport, BicError, BindingItem, BindingPackage, BindingType, CallingConvention,
    FunctionBinding, HeaderConfig, LinkResolutionMode, MacroBinding, MacroCategory, MacroForm,
    MacroKind, MacroValue, ParameterBinding, TypeAliasBinding, TypeLayout, probe_type_layouts,
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
