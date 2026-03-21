//! Shared test-support module for parc-dependent test pipelines.
//!
//! This module provides adapter functions and a `process()` pipeline that
//! connects parc (preprocessing + parsing) to linc. It is compiled only as
//! test support (via `mod common;` in integration tests) and is never part
//! of the linc library itself.

#![allow(dead_code)]

use linc::raw_headers::{
    attach_canonical_alias_resolution, build_effective_macro_environment, build_item_provenance,
    RecoveredParse,
};
use linc::{
    BindingPackage, Diagnostic, DiagnosticKind, HeaderConfig, LincError, PreprocessingReport,
    RawHeaderResult,
};

// ─── parc adapter functions ───────────────────────────────────────────

/// Convert a parc `SourceType` into a linc `BindingType`.
pub fn parc_type_to_binding(ty: &parc::ir::SourceType) -> linc::BindingType {
    use parc::ir::SourceType as PT;
    match ty {
        PT::Void => linc::BindingType::Void,
        PT::Bool => linc::BindingType::Bool,
        PT::Char => linc::BindingType::Char,
        PT::SChar => linc::BindingType::SChar,
        PT::UChar => linc::BindingType::UChar,
        PT::Short => linc::BindingType::Short,
        PT::UShort => linc::BindingType::UShort,
        PT::Int => linc::BindingType::Int,
        PT::UInt => linc::BindingType::UInt,
        PT::Long => linc::BindingType::Long,
        PT::ULong => linc::BindingType::ULong,
        PT::LongLong => linc::BindingType::LongLong,
        PT::ULongLong => linc::BindingType::ULongLong,
        PT::Float => linc::BindingType::Float,
        PT::Double => linc::BindingType::Double,
        PT::LongDouble => linc::BindingType::LongDouble,
        PT::Int128 | PT::UInt128 => linc::BindingType::Opaque(format!("{ty:?}")),
        PT::Pointer {
            pointee,
            qualifiers,
        } => linc::BindingType::Pointer {
            pointee: Box::new(parc_type_to_binding(pointee)),
            const_pointee: qualifiers.is_const,
            qualifiers: linc::ir::TypeQualifiers {
                is_const: false,
                is_volatile: qualifiers.is_volatile,
                is_restrict: qualifiers.is_restrict,
                is_atomic: qualifiers.is_atomic,
            },
        },
        PT::Array(elem, size) => {
            linc::BindingType::Array(Box::new(parc_type_to_binding(elem)), *size)
        }
        PT::Qualified {
            ty: inner,
            qualifiers,
        } => {
            let base = parc_type_to_binding(inner);
            let q = linc::ir::TypeQualifiers {
                is_const: qualifiers.is_const,
                is_volatile: qualifiers.is_volatile,
                is_restrict: qualifiers.is_restrict,
                is_atomic: qualifiers.is_atomic,
            };
            linc::BindingType::Qualified {
                ty: Box::new(base),
                qualifiers: q,
            }
        }
        PT::FunctionPointer {
            return_type,
            parameters,
            variadic,
        } => linc::BindingType::FunctionPointer {
            return_type: Box::new(parc_type_to_binding(return_type)),
            parameters: parameters.iter().map(parc_type_to_binding).collect(),
            variadic: *variadic,
        },
        PT::TypedefRef(name) => linc::BindingType::TypedefRef(name.clone()),
        PT::RecordRef(name) => linc::BindingType::RecordRef(name.clone()),
        PT::EnumRef(name) => linc::BindingType::EnumRef(name.clone()),
        PT::Opaque(name) => linc::BindingType::Opaque(name.clone()),
    }
}

/// Convert a parc `SourceItem` into a linc `BindingItem`.
pub fn parc_item_to_binding(item: &parc::ir::SourceItem) -> Option<linc::BindingItem> {
    use parc::ir::SourceItem as PI;
    match item {
        PI::Function(f) => Some(linc::BindingItem::Function(linc::FunctionBinding {
            name: f.name.clone(),
            calling_convention: linc::CallingConvention::C,
            parameters: f
                .parameters
                .iter()
                .map(|p| linc::ParameterBinding {
                    name: p.name.clone(),
                    ty: parc_type_to_binding(&p.ty),
                })
                .collect(),
            return_type: parc_type_to_binding(&f.return_type),
            variadic: f.variadic,
            source_offset: f.source_offset,
        })),
        PI::Record(r) => Some(linc::BindingItem::Record(linc::RecordBinding {
            kind: match r.kind {
                parc::ir::RecordKind::Struct => linc::ir::RecordKind::Struct,
                parc::ir::RecordKind::Union => linc::ir::RecordKind::Union,
            },
            name: r.name.clone(),
            fields: r.fields.as_ref().map(|fields| {
                fields
                    .iter()
                    .map(|f| linc::FieldBinding {
                        name: f.name.clone(),
                        ty: parc_type_to_binding(&f.ty),
                        bit_width: f.bit_width,
                        layout: None,
                    })
                    .collect()
            }),
            source_offset: r.source_offset,
            representation: None,
            abi_confidence: None,
        })),
        PI::Enum(e) => Some(linc::BindingItem::Enum(linc::EnumBinding {
            name: e.name.clone(),
            variants: e
                .variants
                .iter()
                .map(|v| linc::EnumVariant {
                    name: v.name.clone(),
                    value: v.value,
                })
                .collect(),
            source_offset: e.source_offset,
            representation: None,
            abi_confidence: None,
        })),
        PI::TypeAlias(t) => Some(linc::BindingItem::TypeAlias(linc::TypeAliasBinding {
            name: t.name.clone(),
            target: parc_type_to_binding(&t.target),
            source_offset: t.source_offset,
            canonical_resolution: None,
            abi_confidence: None,
        })),
        PI::Variable(v) => Some(linc::BindingItem::Variable(linc::VariableBinding {
            name: v.name.clone(),
            ty: parc_type_to_binding(&v.ty),
            source_offset: v.source_offset,
        })),
        PI::Unsupported(u) => Some(linc::BindingItem::Unsupported(linc::ir::UnsupportedItem {
            name: u.name.clone(),
            reason: u.reason.clone(),
            source_offset: None,
        })),
    }
}

/// Convert a parc `SourcePackage` into a linc `BindingPackage`.
pub fn from_parc_package(src: &parc::ir::SourcePackage) -> BindingPackage {
    let items: Vec<linc::BindingItem> = src.items.iter().filter_map(parc_item_to_binding).collect();
    BindingPackage {
        source_path: src.source_path.clone(),
        items,
        diagnostics: Vec::new(),
        ..BindingPackage::new()
    }
}

// ─── Flavor conversion ───────────────────────────────────────────────

/// Convert a linc `Flavor` to a parc `Flavor`.
pub fn to_pac(flavor: linc::raw_headers::Flavor) -> parc::driver::Flavor {
    match flavor {
        linc::raw_headers::Flavor::GnuC11 => parc::driver::Flavor::GnuC11,
        linc::raw_headers::Flavor::ClangC11 => parc::driver::Flavor::ClangC11,
        linc::raw_headers::Flavor::StdC11 => parc::driver::Flavor::StdC11,
    }
}

// ─── parc driver configuration ───────────────────────────────────────

/// Build a `parc::driver::Config` from a `HeaderConfig`.
pub fn build_pac_config(config: &HeaderConfig) -> parc::driver::Config {
    let flavor = config.flavor.unwrap_or(linc::raw_headers::Flavor::GnuC11);
    let compiler = config.compiler.clone().unwrap_or_else(|| match flavor {
        linc::raw_headers::Flavor::ClangC11 => "clang".into(),
        _ => "gcc".into(),
    });

    let mut cpp_options = vec!["-E".to_string()];

    for dir in &config.include_dirs {
        cpp_options.push(format!("-I{}", dir.display()));
    }

    for (name, value) in &config.defines {
        match value {
            Some(v) => cpp_options.push(format!("-D{}={}", name, v)),
            None => cpp_options.push(format!("-D{}", name)),
        }
    }

    parc::driver::Config {
        cpp_command: compiler,
        cpp_options,
        flavor: to_pac(flavor),
    }
}

/// Build the (command, args) invocation description from a parc config.
fn describe_invocation(
    config: &parc::driver::Config,
    input: &std::path::Path,
) -> (String, Vec<String>) {
    let command = config.cpp_command.clone();
    let mut args = config.cpp_options.clone();
    args.push(input.display().to_string());
    (command, args)
}

/// Try to recover items from preprocessed source after a syntax error.
fn try_recover_items_from_preprocessed_source(
    header_config: &HeaderConfig,
    source: &str,
) -> Option<RecoveredParse> {
    let sanitized_source = linc::raw_headers::sanitize_attribute_bearing_record_typedefs(source)?;
    let flavor = to_pac(
        header_config
            .flavor
            .unwrap_or(linc::raw_headers::Flavor::GnuC11),
    );
    let unit = parc::parse::translation_unit(&sanitized_source, flavor).ok()?;
    let src_pkg = parc::extract::extract_from_translation_unit(&unit, None);
    let converted = from_parc_package(&src_pkg);
    let items = converted.items;
    let mut diagnostics = converted.diagnostics;
    diagnostics.push(Diagnostic::warning(
        DiagnosticKind::DeclarationPartial,
        "recovered declaration extraction after sanitizing packed record typedef attributes",
    ));
    Some(RecoveredParse {
        source: sanitized_source,
        items,
        diagnostics,
    })
}

/// Build a package from a recovered parse result.
fn package_from_recovered_parse(
    config: &HeaderConfig,
    recovered: RecoveredParse,
    macros: Vec<linc::MacroBinding>,
    macro_provenance: Vec<linc::ir::MacroProvenance>,
    report: PreprocessingReport,
) -> Result<RawHeaderResult, LincError> {
    let source_desc = config
        .entry_headers
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let effective_macro_environment =
        build_effective_macro_environment(&macros, &macro_provenance);

    let mut package = BindingPackage {
        source_path: Some(source_desc),
        target: config.binding_target(),
        inputs: config.binding_inputs(),
        macros,
        macro_provenance,
        effective_macro_environment,
        link: config.binding_link_surface(),
        items: recovered.items,
        diagnostics: recovered.diagnostics,
        ..BindingPackage::new()
    };

    let origin_map =
        linc::line_markers::FileOriginMap::parse(&recovered.source, &config.entry_headers);
    package.provenance = build_item_provenance(&package.items, &origin_map);
    attach_canonical_alias_resolution(&mut package.items);

    config.attach_requested_probes(&mut package)?;

    if let Some(ref filter) = config.origin_filter {
        package.filter_by_origin(&origin_map, filter);
    }

    Ok(RawHeaderResult { package, report })
}

// ─── Main process pipeline ───────────────────────────────────────────

/// Run the full header scanning pipeline using parc for preprocessing
/// and parsing. This replicates the logic that was formerly
/// `HeaderConfig::process()`.
pub fn process(config: &HeaderConfig) -> Result<RawHeaderResult, LincError> {
    config.validate()?;

    // Build a combined header source that includes all entry headers
    let combined = config.build_combined_source();
    let unique_id = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_dir = std::env::temp_dir().join(format!("linc_raw_{unique_id}_{ts}"));
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join("_linc_combined.c");
    std::fs::write(&tmp_file, &combined)?;

    let pac_config = build_pac_config(config);
    let (command, args) = describe_invocation(&pac_config, &tmp_file);

    let parse_result = parc::driver::parse(&pac_config, &tmp_file);
    let (macros, macro_provenance) = config.capture_macros(&tmp_file);

    // Clean up
    std::fs::remove_file(&tmp_file).ok();
    std::fs::remove_dir(&tmp_dir).ok();

    match parse_result {
        Ok(parsed) => {
            let report = PreprocessingReport {
                command,
                args,
                preprocessed_source: parsed.source.clone(),
            };

            let src_pkg = parc::extract::extract_from_translation_unit(&parsed.unit, None);
            let converted = from_parc_package(&src_pkg);
            let items = converted.items;
            let diagnostics = converted.diagnostics;

            let source_desc = config
                .entry_headers
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let effective_macro_environment =
                build_effective_macro_environment(&macros, &macro_provenance);

            let mut package = BindingPackage {
                source_path: Some(source_desc),
                target: config.binding_target(),
                inputs: config.binding_inputs(),
                macros,
                macro_provenance,
                effective_macro_environment,
                link: config.binding_link_surface(),
                items,
                diagnostics,
                ..BindingPackage::new()
            };

            let origin_map =
                linc::line_markers::FileOriginMap::parse(&parsed.source, &config.entry_headers);
            package.provenance = build_item_provenance(&package.items, &origin_map);
            attach_canonical_alias_resolution(&mut package.items);

            config.attach_requested_probes(&mut package)?;

            // Apply origin filtering if configured
            if let Some(ref filter) = config.origin_filter {
                package.filter_by_origin(&origin_map, filter);
            }

            Ok(RawHeaderResult { package, report })
        }
        Err(parc::driver::Error::PreprocessorError(e)) => {
            let mut pkg = BindingPackage::new();
            pkg.diagnostics.push(Diagnostic::error(
                DiagnosticKind::PreprocessingFailed,
                format!("preprocessor failed: {}", e),
            ));
            Ok(RawHeaderResult {
                package: pkg,
                report: PreprocessingReport {
                    command,
                    args,
                    preprocessed_source: String::new(),
                },
            })
        }
        Err(parc::driver::Error::SyntaxError(e)) => {
            if let Some(recovered) =
                try_recover_items_from_preprocessed_source(config, &e.source)
            {
                let report = PreprocessingReport {
                    command,
                    args,
                    preprocessed_source: e.source,
                };
                return package_from_recovered_parse(
                    config,
                    recovered,
                    macros,
                    macro_provenance,
                    report,
                );
            }

            let source_desc = config
                .entry_headers
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let effective_macro_environment =
                build_effective_macro_environment(&macros, &macro_provenance);
            let mut pkg = BindingPackage {
                source_path: Some(source_desc),
                target: config.binding_target(),
                inputs: config.binding_inputs(),
                macros,
                macro_provenance,
                effective_macro_environment,
                link: config.binding_link_surface(),
                ..BindingPackage::new()
            };
            pkg.diagnostics.push(Diagnostic::error(
                DiagnosticKind::ParseFailed,
                format!("parse error: {}", e),
            ));

            config.attach_requested_probes(&mut pkg)?;

            Ok(RawHeaderResult {
                package: pkg,
                report: PreprocessingReport {
                    command,
                    args,
                    preprocessed_source: e.source,
                },
            })
        }
    }
}
