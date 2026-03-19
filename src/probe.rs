use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::BicError;
use crate::ir::{BindingTarget, TypeLayout};
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
    pub layout: TypeLayout,
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
    let temp_root = temp_probe_root();
    std::fs::create_dir_all(&temp_root)?;
    let source_path = temp_root.join("probe.c");
    let exe_path = temp_root.join("probe-bin");

    std::fs::write(&source_path, build_probe_source(config, type_names))?;

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

fn build_probe_source(config: &HeaderConfig, type_names: &[impl AsRef<str>]) -> String {
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
                "    printf(\"%s\\t%zu\\t%zu\\t%zu\\t%d\\n\", \"{}\", sizeof({}), _Alignof({}), sizeof({}), (({})-1) < (({})0) ? 1 : 0);\n",
                literal, raw, raw, raw, raw, raw
            )),
            _ => source.push_str(&format!(
                "    printf(\"%s\\t%zu\\t%zu\\t%s\\t%s\\n\", \"{}\", sizeof({}), _Alignof({}), \"-\", \"-\");\n",
                literal, raw, raw
            )),
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
}

fn parse_layout_output(stdout: &str) -> Result<Vec<ParsedProbeLayout>, BicError> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.split('\t');
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
            Ok(ParsedProbeLayout {
                layout: TypeLayout {
                    name: name.to_string(),
                    size,
                    align,
                },
                enum_underlying_size,
                enum_is_signed,
            })
        })
        .collect()
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
                },
                ParsedProbeLayout {
                    layout: TypeLayout {
                        name: "value_t".into(),
                        size: 4,
                        align: 4,
                    },
                    enum_underlying_size: None,
                    enum_is_signed: None,
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
        assert_eq!(report.layouts.len(), 1);
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
    }

    #[test]
    fn parse_layout_output_captures_enum_representation() {
        let parsed = parse_layout_output("enum mode\t4\t4\t4\t1\n").unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].layout.name, "enum mode");
        assert_eq!(parsed[0].enum_underlying_size, Some(4));
        assert_eq!(parsed[0].enum_is_signed, Some(true));
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
}
