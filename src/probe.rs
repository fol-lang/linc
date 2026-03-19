use std::path::PathBuf;

use pac::ast::TranslationUnit;
use serde::{Deserialize, Serialize};

use crate::error::BicError;
use crate::extract::Extractor;
use crate::ir::{BindingItem, BindingTarget, TypeLayout};
use crate::raw_headers::{Flavor, HeaderConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeSubjectKind {
    Type,
    Record,
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeConfidence {
    MeasuredLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordCompleteness {
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeSubjectReport {
    pub name: String,
    pub kind: ProbeSubjectKind,
    #[serde(default = "default_probe_confidence")]
    pub confidence: ProbeConfidence,
    #[serde(default)]
    pub record_completeness: Option<RecordCompleteness>,
    #[serde(default)]
    pub enum_underlying_size: Option<u64>,
    #[serde(default)]
    pub enum_is_signed: Option<bool>,
    #[serde(default)]
    pub fields: Vec<ProbedFieldLayout>,
    pub layout: TypeLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbedFieldLayout {
    pub name: String,
    #[serde(default)]
    pub offset_bytes: Option<u64>,
    #[serde(default)]
    pub bit_width: Option<u64>,
}

/// Result of an ABI layout probe run.
///
/// Invariant: `layouts` is only populated for subjects that were explicitly requested and
/// successfully compiled/executed under the captured target/compiler identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiProbeReport {
    #[serde(default)]
    pub target: BindingTarget,
    pub compiler_command: String,
    pub entry_headers: Vec<String>,
    #[serde(default)]
    pub subjects: Vec<ProbeSubjectReport>,
    pub layouts: Vec<TypeLayout>,
}

pub fn probe_type_layouts(
    config: &HeaderConfig,
    type_names: &[impl AsRef<str>],
) -> Result<AbiProbeReport, BicError> {
    config.validate()?;
    if type_names.is_empty() {
        return Err(BicError::NoProbeTypes);
    }

    let compiler = compiler_command(config);
    let field_specs = collect_record_field_specs(config);
    let temp_root = temp_probe_root();
    std::fs::create_dir_all(&temp_root)?;
    let source_path = temp_root.join("probe.c");
    let exe_path = temp_root.join("probe-bin");

    std::fs::write(&source_path, build_probe_source(config, type_names, &field_specs))?;

    let mut compile = std::process::Command::new(&compiler);
    compile.arg("-std=c11");
    for dir in &config.include_dirs {
        compile.arg(format!("-I{}", dir.display()));
    }
    for (name, value) in &config.defines {
        match value {
            Some(v) => {
                compile.arg(format!("-D{}={}", name, v));
            }
            None => {
                compile.arg(format!("-D{}", name));
            }
        }
    }
    compile.arg(&source_path).arg("-o").arg(&exe_path);

    let compile_output = compile
        .output()
        .map_err(|e| BicError::ProbeCompile {
            compiler: compiler.clone(),
            stderr: e.to_string(),
        })?;
    if !compile_output.status.success() {
        let stderr = String::from_utf8_lossy(&compile_output.stderr);
        cleanup_probe_root(&temp_root);
        return Err(BicError::ProbeCompile {
            compiler,
            stderr: stderr.trim().to_string(),
        });
    }

    let run_output = std::process::Command::new(&exe_path)
        .output()
        .map_err(|e| BicError::ProbeExecution {
            reason: e.to_string(),
        })?;
    if !run_output.status.success() {
        let stderr = String::from_utf8_lossy(&run_output.stderr);
        cleanup_probe_root(&temp_root);
        return Err(BicError::ProbeExecution {
            reason: stderr.trim().to_string(),
        });
    }

    let stdout = String::from_utf8(run_output.stdout)
        .map_err(|e| BicError::ProbeOutput {
            reason: e.to_string(),
        })?;
    let parsed = parse_layout_output(&stdout)?;
    let layouts = parsed
        .iter()
        .map(|entry| entry.layout.clone())
        .collect::<Vec<_>>();
    let subjects = type_names
        .iter()
        .zip(parsed.iter())
        .map(|(type_name, parsed)| ProbeSubjectReport {
            name: type_name.as_ref().to_string(),
            kind: classify_probe_subject(type_name.as_ref()),
            confidence: ProbeConfidence::MeasuredLayout,
            record_completeness: classify_record_completeness(type_name.as_ref()),
            enum_underlying_size: parsed.enum_underlying_size,
            enum_is_signed: parsed.enum_is_signed,
            fields: parsed.fields.clone(),
            layout: parsed.layout.clone(),
        })
        .collect();
    cleanup_probe_root(&temp_root);

    Ok(AbiProbeReport {
        target: config.binding_target(),
        compiler_command: compiler,
        entry_headers: config
            .entry_headers
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        subjects,
        layouts,
    })
}

fn classify_probe_subject(type_name: &str) -> ProbeSubjectKind {
    if type_name.trim_start().starts_with("struct ")
        || type_name.trim_start().starts_with("union ")
    {
        ProbeSubjectKind::Record
    } else if type_name.trim_start().starts_with("enum ") {
        ProbeSubjectKind::Enum
    } else {
        ProbeSubjectKind::Type
    }
}

fn classify_record_completeness(type_name: &str) -> Option<RecordCompleteness> {
    matches!(classify_probe_subject(type_name), ProbeSubjectKind::Record)
        .then_some(RecordCompleteness::Complete)
}

fn default_probe_confidence() -> ProbeConfidence {
    ProbeConfidence::MeasuredLayout
}

fn compiler_command(config: &HeaderConfig) -> String {
    let flavor = config.flavor.unwrap_or(Flavor::GnuC11);
    config
        .compiler
        .clone()
        .unwrap_or_else(|| match flavor {
            Flavor::ClangC11 => "clang".into(),
            _ => "gcc".into(),
        })
}

fn build_probe_source(
    config: &HeaderConfig,
    type_names: &[impl AsRef<str>],
    field_specs: &std::collections::BTreeMap<String, Vec<ProbedFieldSpec>>,
) -> String {
    let mut source = String::from("#include <stdio.h>\n#include <stddef.h>\n");
    for header in &config.entry_headers {
        source.push_str(&format!("#include \"{}\"\n", header.display()));
    }
    source.push_str("\nint main(void) {\n");
    for type_name in type_names {
        let raw = type_name.as_ref();
        let literal = c_string_literal(raw);
        match classify_probe_subject(raw) {
            ProbeSubjectKind::Enum => source.push_str(&format!(
                "    printf(\"L\\t%s\\t%zu\\t%zu\\t%zu\\t%d\\n\", \"{}\", sizeof({}), _Alignof({}), sizeof({}), (({})-1) < (({})0) ? 1 : 0);\n",
                literal, raw, raw, raw, raw, raw
            )),
            _ => source.push_str(&format!(
                "    printf(\"L\\t%s\\t%zu\\t%zu\\t%s\\t%s\\n\", \"{}\", sizeof({}), _Alignof({}), \"-\", \"-\");\n",
                literal, raw, raw
            )),
        }
        if let Some(fields) = field_specs.get(raw) {
            for field in fields {
                let field_literal = c_string_literal(&field.name);
                if let Some(bit_width) = field.bit_width {
                    source.push_str(&format!(
                        "    printf(\"F\\t%s\\t%s\\t-\\t%zu\\n\", \"{}\", \"{}\", (size_t){});\n",
                        literal, field_literal, bit_width
                    ));
                } else {
                    source.push_str(&format!(
                        "    printf(\"F\\t%s\\t%s\\t%zu\\t-\\n\", \"{}\", \"{}\", offsetof({}, {}));\n",
                        literal, field_literal, raw, field.name
                    ));
                }
            }
        }
    }
    source.push_str("    return 0;\n}\n");
    source
}

fn c_string_literal(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedProbeLayout {
    layout: TypeLayout,
    enum_underlying_size: Option<u64>,
    enum_is_signed: Option<bool>,
    fields: Vec<ProbedFieldLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbedFieldSpec {
    name: String,
    bit_width: Option<u64>,
}

fn parse_layout_output(stdout: &str) -> Result<Vec<ParsedProbeLayout>, BicError> {
    let mut entries = Vec::<ParsedProbeLayout>::new();
    let mut entry_indexes = std::collections::HashMap::<String, usize>::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let mut parts = line.split('\t');
        match parts.next() {
            Some("L") => {
                let name = parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid probe output line: {}", line),
                    })?;
                let size = parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid probe output line: {}", line),
                    })?
                    .parse::<u64>()
                    .map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid size in probe output '{}': {}", line, e),
                    })?;
                let align = parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid probe output line: {}", line),
                    })?
                    .parse::<u64>()
                    .map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid align in probe output '{}': {}", line, e),
                    })?;
                let enum_underlying_size = match parts.next() {
                    Some("-") | None => None,
                    Some(value) => Some(value.parse::<u64>().map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid enum size in probe output '{}': {}", line, e),
                    })?),
                };
                let enum_is_signed = match parts.next() {
                    Some("-") | None => None,
                    Some("0") => Some(false),
                    Some("1") => Some(true),
                    Some(value) => {
                        return Err(BicError::ProbeOutput {
                            reason: format!(
                                "invalid enum signedness in probe output '{}': {}",
                                line, value
                            ),
                        })
                    }
                };
                entry_indexes.insert(name.to_string(), entries.len());
                entries.push(ParsedProbeLayout {
                    layout: TypeLayout {
                        name: name.to_string(),
                        size,
                        align,
                    },
                    enum_underlying_size,
                    enum_is_signed,
                    fields: Vec::new(),
                });
            }
            Some("F") => {
                let subject = parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid field probe output line: {}", line),
                    })?;
                let field_name = parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid field probe output line: {}", line),
                    })?;
                let offset_bytes = match parts.next() {
                    Some("-") => None,
                    Some(value) => Some(value.parse::<u64>().map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid field offset in probe output '{}': {}", line, e),
                    })?),
                    None => {
                        return Err(BicError::ProbeOutput {
                            reason: format!("invalid field probe output line: {}", line),
                        })
                    }
                };
                let bit_width = match parts.next() {
                    Some("-") | None => None,
                    Some(value) => Some(value.parse::<u64>().map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid bitfield width in probe output '{}': {}", line, e),
                    })?),
                };
                let entry = entry_indexes
                    .get(subject)
                    .copied()
                    .and_then(|index| entries.get_mut(index))
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!(
                            "field probe output '{}' referenced unknown subject '{}'",
                            line, subject
                        ),
                    })?;
                entry.fields.push(ProbedFieldLayout {
                    name: field_name.to_string(),
                    offset_bytes,
                    bit_width,
                });
            }
            Some(other) => {
                let mut legacy_parts = line.split('\t');
                let name = if other == line {
                    other
                } else {
                    legacy_parts
                        .next()
                        .ok_or_else(|| BicError::ProbeOutput {
                            reason: format!("invalid probe output line: {}", line),
                        })?
                };
                let size = legacy_parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid probe output line: {}", line),
                    })?
                    .parse::<u64>()
                    .map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid size in probe output '{}': {}", line, e),
                    })?;
                let align = legacy_parts
                    .next()
                    .ok_or_else(|| BicError::ProbeOutput {
                        reason: format!("invalid probe output line: {}", line),
                    })?
                    .parse::<u64>()
                    .map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid align in probe output '{}': {}", line, e),
                    })?;
                let enum_underlying_size = match legacy_parts.next() {
                    Some("-") | None => None,
                    Some(value) => Some(value.parse::<u64>().map_err(|e| BicError::ProbeOutput {
                        reason: format!("invalid enum size in probe output '{}': {}", line, e),
                    })?),
                };
                let enum_is_signed = match legacy_parts.next() {
                    Some("-") | None => None,
                    Some("0") => Some(false),
                    Some("1") => Some(true),
                    Some(value) => {
                        return Err(BicError::ProbeOutput {
                            reason: format!(
                                "invalid enum signedness in probe output '{}': {}",
                                line, value
                            ),
                        })
                    }
                };
                entry_indexes.insert(name.to_string(), entries.len());
                entries.push(ParsedProbeLayout {
                    layout: TypeLayout {
                        name: name.to_string(),
                        size,
                        align,
                    },
                    enum_underlying_size,
                    enum_is_signed,
                    fields: Vec::new(),
                });
            }
            None => {}
        }
    }
    Ok(entries)
}

fn collect_record_field_specs(
    config: &HeaderConfig,
) -> std::collections::BTreeMap<String, Vec<ProbedFieldSpec>> {
    let unit = match parse_probe_translation_unit(config) {
        Some(unit) => unit,
        None => return std::collections::BTreeMap::new(),
    };
    let extractor = Extractor::new();
    let (items, _) = extractor.extract(&unit);
    let mut fields = std::collections::BTreeMap::new();
    for item in items {
        if let BindingItem::Record(record) = item {
            let key = match (record.kind, record.name.as_deref()) {
                (_, None) => continue,
                (crate::ir::RecordKind::Struct, Some(name)) => format!("struct {}", name),
                (crate::ir::RecordKind::Union, Some(name)) => format!("union {}", name),
            };
            let named_fields = record
                .fields
                .unwrap_or_default()
                .into_iter()
                .filter_map(|field| {
                    Some(ProbedFieldSpec {
                        name: field.name?,
                        bit_width: field.bit_width,
                    })
                })
                .collect::<Vec<_>>();
            if !named_fields.is_empty() {
                fields.insert(key, named_fields);
            }
        }
    }
    fields
}

fn parse_probe_translation_unit(config: &HeaderConfig) -> Option<TranslationUnit> {
    let combined = config
        .entry_headers
        .iter()
        .map(|header| format!("#include \"{}\"\n", header.display()))
        .collect::<String>();
    let tmp_root = temp_probe_root().join("parse");
    std::fs::create_dir_all(&tmp_root).ok()?;
    let tmp_file = tmp_root.join("_bic_probe_fields.c");
    std::fs::write(&tmp_file, combined).ok()?;
    let compiler = compiler_command(config);
    let flavor = config.flavor.unwrap_or(Flavor::GnuC11);
    let mut cpp_options = vec!["-E".to_string()];
    for dir in &config.include_dirs {
        cpp_options.push(format!("-I{}", dir.display()));
    }
    for (name, value) in &config.defines {
        match value {
            Some(value) => cpp_options.push(format!("-D{}={}", name, value)),
            None => cpp_options.push(format!("-D{}", name)),
        }
    }
    let result = pac::driver::parse(
        &pac::driver::Config {
            cpp_command: compiler,
            cpp_options,
            flavor: match flavor {
                Flavor::GnuC11 => pac::driver::Flavor::GnuC11,
                Flavor::ClangC11 => pac::driver::Flavor::ClangC11,
                Flavor::StdC11 => pac::driver::Flavor::StdC11,
            },
        },
        &tmp_file,
    )
    .ok()
    .map(|parsed| parsed.unit);
    cleanup_probe_root(&tmp_root);
    result
}

fn temp_probe_root() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("bic_probe_{}_{}", std::process::id(), id))
}

fn cleanup_probe_root(root: &std::path::Path) {
    std::fs::remove_dir_all(root).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bic_probe_test_{label}_{}_{}", std::process::id(), id));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_layout_output_roundtrip() {
        let parsed = parse_layout_output("widget\t16\t8\nvalue_t\t4\t4\n").unwrap();
        assert_eq!(
            parsed,
            vec![
                ParsedProbeLayout {
                    layout: TypeLayout {
                        name: "widget".into(),
                        size: 16,
                        align: 8,
                    },
                    enum_underlying_size: None,
                    enum_is_signed: None,
                    fields: Vec::new(),
                },
                ParsedProbeLayout {
                    layout: TypeLayout {
                        name: "value_t".into(),
                        size: 4,
                        align: 4,
                    },
                    enum_underlying_size: None,
                    enum_is_signed: None,
                    fields: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn probe_type_layouts_from_header() {
        let dir = temp_dir("header");
        let header = dir.join("api.h");
        std::fs::write(
            &header,
            "typedef unsigned int value_t;\nstruct widget { int a; double b; };\n",
        )
        .unwrap();

        let report = probe_type_layouts(
            &HeaderConfig::new().header(&header),
            &["value_t", "struct widget"],
        )
        .unwrap();

        assert_eq!(report.target.compiler_command.as_deref(), Some("gcc"));
        assert_eq!(report.target.flavor.as_deref(), Some("gnu-c11"));
        assert_eq!(report.layouts.len(), 2);
        assert_eq!(report.subjects.len(), 2);
        assert_eq!(report.subjects[0].kind, ProbeSubjectKind::Type);
        assert_eq!(report.subjects[1].kind, ProbeSubjectKind::Record);
        assert_eq!(report.subjects[0].confidence, ProbeConfidence::MeasuredLayout);
        assert_eq!(
            report.subjects[1].record_completeness,
            Some(RecordCompleteness::Complete)
        );
        assert_eq!(report.subjects[1].fields.len(), 2);
        assert_eq!(report.subjects[1].fields[0].name, "a");
        assert_eq!(report.subjects[1].fields[0].offset_bytes, Some(0));
        assert_eq!(report.subjects[1].fields[1].name, "b");
        assert!(report.layouts.iter().any(|layout| {
            layout.name == "value_t" && layout.size >= 4 && layout.align >= 4
        }));
        assert!(report.layouts.iter().any(|layout| {
            layout.name == "struct widget" && layout.size >= 16 && layout.align >= 8
        }));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn probe_requires_type_names() {
        let dir = temp_dir("empty_probe");
        let header = dir.join("api.h");
        std::fs::write(&header, "struct widget { int a; };\n").unwrap();

        let err = probe_type_layouts(&HeaderConfig::new().header(&header), &[] as &[&str])
            .unwrap_err();
        assert!(matches!(err, BicError::NoProbeTypes));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn probe_rejects_invalid_config_before_execution() {
        let err = probe_type_layouts(
            &HeaderConfig::new().header("").probe_type_layout("size_t"),
            &["size_t"],
        )
        .unwrap_err();
        assert!(matches!(err, BicError::InvalidConfig { .. }));
    }

    #[test]
    fn probe_report_serialization_preserves_target_identity() {
        let report = AbiProbeReport {
            target: BindingTarget {
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
    fn probe_report_contract_snapshot_is_consumable() {
        let json = include_str!("../test/contracts/probe_report_contract_snapshot.json");
        let report: AbiProbeReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.compiler_command, "clang");
        assert_eq!(report.target.compiler_command.as_deref(), Some("clang"));
        assert_eq!(report.subjects.len(), 1);
        assert_eq!(report.subjects[0].name, "size_t");
        assert!(report.subjects[0].fields.is_empty());
        assert_eq!(report.layouts.len(), 1);
    }

    #[test]
    fn probe_record_contract_snapshot_is_consumable() {
        let json = include_str!("../test/contracts/probe_record_contract_snapshot.json");
        let report: AbiProbeReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.subjects.len(), 2);
        assert_eq!(report.subjects[0].kind, ProbeSubjectKind::Record);
        assert_eq!(
            report.subjects[0].record_completeness,
            Some(RecordCompleteness::Complete)
        );
        assert_eq!(report.subjects[1].kind, ProbeSubjectKind::Enum);
        assert_eq!(report.subjects[1].enum_underlying_size, Some(4));
        assert_eq!(report.subjects[1].enum_is_signed, Some(true));
    }

    #[test]
    fn probe_subject_kind_classification_handles_records_and_enums() {
        assert_eq!(classify_probe_subject("size_t"), ProbeSubjectKind::Type);
        assert_eq!(
            classify_probe_subject("struct widget"),
            ProbeSubjectKind::Record
        );
        assert_eq!(
            classify_probe_subject(" union payload"),
            ProbeSubjectKind::Record
        );
        assert_eq!(classify_probe_subject("enum mode"), ProbeSubjectKind::Enum);
    }

    #[test]
    fn probe_report_defaults_confidence_for_older_json() {
        let json = r#"{
          "target": {},
          "compiler_command": "clang",
          "entry_headers": ["demo.h"],
          "subjects": [
            {
              "name": "struct widget",
              "kind": "Record",
              "layout": { "name": "struct widget", "size": 16, "align": 8 }
            }
          ],
          "layouts": [{ "name": "struct widget", "size": 16, "align": 8 }]
        }"#;
        let report: AbiProbeReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.subjects[0].confidence, ProbeConfidence::MeasuredLayout);
        assert_eq!(report.subjects[0].record_completeness, None);
        assert!(report.subjects[0].fields.is_empty());
    }

    #[test]
    fn parse_layout_output_captures_enum_representation() {
        let parsed = parse_layout_output("enum mode\t4\t4\t4\t1\n").unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].layout.name, "enum mode");
        assert_eq!(parsed[0].enum_underlying_size, Some(4));
        assert_eq!(parsed[0].enum_is_signed, Some(true));
        assert!(parsed[0].fields.is_empty());
    }

    #[test]
    fn parse_layout_output_captures_field_offsets() {
        let parsed = parse_layout_output(
            "L\tstruct widget\t16\t8\t-\t-\nF\tstruct widget\tx\t0\nF\tstruct widget\ty\t8\n",
        )
        .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].fields.len(), 2);
        assert_eq!(parsed[0].fields[0].name, "x");
        assert_eq!(parsed[0].fields[0].offset_bytes, Some(0));
    }

    #[test]
    fn parse_layout_output_captures_partial_bitfield_probe_data() {
        let parsed = parse_layout_output(
            "L\tstruct flags\t4\t4\t-\t-\nF\tstruct flags\tvalue\t-\t3\n",
        )
        .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].fields.len(), 1);
        assert_eq!(parsed[0].fields[0].name, "value");
        assert_eq!(parsed[0].fields[0].offset_bytes, None);
        assert_eq!(parsed[0].fields[0].bit_width, Some(3));
    }

    #[test]
    fn probe_type_layouts_reports_enum_underlying_representation() {
        let dir = temp_dir("enum_header");
        let header = dir.join("api.h");
        std::fs::write(&header, "enum mode { MODE_A = 0, MODE_B = 7 };\n").unwrap();

        let report = probe_type_layouts(&HeaderConfig::new().header(&header), &["enum mode"])
            .unwrap();

        assert_eq!(report.subjects.len(), 1);
        assert_eq!(report.subjects[0].kind, ProbeSubjectKind::Enum);
        assert_eq!(report.subjects[0].enum_underlying_size, Some(report.layouts[0].size));
        assert!(report.subjects[0].enum_is_signed.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn probe_type_layouts_reports_partial_bitfield_field_data() {
        let dir = temp_dir("bitfield_header");
        let header = dir.join("api.h");
        std::fs::write(&header, "struct flags { unsigned value:3; unsigned other:5; };\n")
            .unwrap();

        let report = probe_type_layouts(&HeaderConfig::new().header(&header), &["struct flags"])
            .unwrap();

        assert_eq!(report.subjects.len(), 1);
        assert_eq!(report.subjects[0].fields.len(), 2);
        assert_eq!(report.subjects[0].fields[0].name, "value");
        assert_eq!(report.subjects[0].fields[0].offset_bytes, None);
        assert_eq!(report.subjects[0].fields[0].bit_width, Some(3));
        assert_eq!(report.subjects[0].fields[1].name, "other");
        assert_eq!(report.subjects[0].fields[1].bit_width, Some(5));

        std::fs::remove_dir_all(&dir).ok();
    }
}
