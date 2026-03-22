//! Intermediate representation for the LINC analysis pipeline.
//!
//! The IR is organized into submodules by concern:
//!
//! - [`link`]: Native link surface types (libraries, artifacts, frameworks)
//! - [`types`]: Declaration and type representation types
//! - [`macros`]: Preprocessor macro types and classification
//!
//! The top-level [`BindingPackage`] ties everything together as the primary
//! machine-readable output.

pub mod link;
pub mod macros;
pub mod types;

// Re-export everything for backward compatibility with `use crate::ir::*`
pub use link::*;
pub use macros::*;
pub use types::*;

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::line_markers::{SourceLocation, SourceOrigin};

pub const SCHEMA_VERSION: u32 = 1;

/// Compiler/target identity captured alongside a produced package or probe report.
///
/// Invariant: all fields are optional evidence and may be absent on older snapshots or when the
/// upstream toolchain does not expose a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BindingTarget {
    #[serde(default)]
    pub target_triple: Option<String>,
    #[serde(default)]
    pub compiler_command: Option<String>,
    #[serde(default)]
    pub compiler_version: Option<String>,
    #[serde(default)]
    pub flavor: Option<String>,
}

/// One preprocessor define as seen by the binding scan.
///
/// Invariant: `name` is the logical macro identifier and `value == None` represents flag-style
/// defines such as `-DDEBUG`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingDefine {
    pub name: String,
    pub value: Option<String>,
}

/// Input provenance for how a package was produced.
///
/// Invariant: these vectors preserve declaration order and are additive metadata rather than a
/// fully normalized build graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BindingInputs {
    #[serde(default)]
    pub entry_headers: Vec<String>,
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub defines: Vec<BindingDefine>,
}

/// Per-declaration source provenance attached at the package layer.
///
/// Invariant: entries are stored in item order and should remain index-aligned with
/// `BindingPackage.items`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeclarationProvenance {
    #[serde(default)]
    pub item_name: Option<String>,
    #[serde(default)]
    pub item_kind: Option<BindingItemKind>,
    #[serde(default)]
    pub source_offset: Option<usize>,
    #[serde(default)]
    pub source_origin: Option<SourceOrigin>,
    #[serde(default)]
    pub source_location: Option<SourceLocation>,
}

/// Primary machine-readable package emitted by LINC.
///
/// Invariant: additive metadata fields default on deserialize so older snapshots remain consumable,
/// while `items` and `diagnostics` remain the core declaration/result surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingPackage {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_linc_version")]
    pub linc_version: String,
    #[serde(default)]
    pub target: BindingTarget,
    #[serde(default)]
    pub inputs: BindingInputs,
    #[serde(default)]
    pub macros: Vec<MacroBinding>,
    #[serde(default)]
    pub layouts: Vec<TypeLayout>,
    #[serde(default)]
    pub link: BindingLinkSurface,
    #[serde(default)]
    pub provenance: Vec<DeclarationProvenance>,
    #[serde(default)]
    pub macro_provenance: Vec<MacroProvenance>,
    #[serde(default)]
    pub effective_macro_environment: Vec<MacroEnvironmentEntry>,
    pub source_path: Option<String>,
    pub items: Vec<BindingItem>,
    pub diagnostics: Vec<Diagnostic>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

fn default_linc_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

impl BindingPackage {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            linc_version: env!("CARGO_PKG_VERSION").to_string(),
            target: BindingTarget::default(),
            inputs: BindingInputs::default(),
            macros: Vec::new(),
            layouts: Vec::new(),
            link: BindingLinkSurface::default(),
            provenance: Vec::new(),
            macro_provenance: Vec::new(),
            effective_macro_environment: Vec::new(),
            source_path: None,
            items: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl Default for BindingPackage {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingPackage {
    /// Filter items by origin using a file-origin map and filter configuration.
    pub fn filter_by_origin(
        &mut self,
        origin_map: &crate::line_markers::FileOriginMap,
        filter: &crate::line_markers::OriginFilter,
    ) {
        let mut filtered_items = Vec::new();
        let mut filtered_provenance = Vec::new();

        for (index, item) in self.items.drain(..).enumerate() {
            let offset = match &item {
                BindingItem::Function(f) => f.source_offset,
                BindingItem::Record(r) => r.source_offset,
                BindingItem::Enum(e) => e.source_offset,
                BindingItem::TypeAlias(t) => t.source_offset,
                BindingItem::Variable(v) => v.source_offset,
                BindingItem::Unsupported(u) => u.source_offset,
            };
            let keep = match offset {
                Some(off) => filter.accepts(&origin_map.origin_at(off)),
                None => true,
            };
            if keep {
                filtered_items.push(item);
                if let Some(prov) = self.provenance.get(index).cloned() {
                    filtered_provenance.push(prov);
                }
            }
        }

        self.items = filtered_items;
        self.provenance = filtered_provenance;
    }

    pub fn diagnostics_count_by_kind(&self) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        for d in &self.diagnostics {
            let key = format!("{:?}", d.kind);
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
            && self.diagnostics.is_empty()
            && self.macros.is_empty()
            && self.layouts.is_empty()
    }

    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    pub fn probe_unavailable_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == DiagnosticKind::ProbeUnavailable)
            .count()
    }

    pub fn probe_failure_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == DiagnosticKind::ProbeFailed)
            .count()
    }

    pub fn has_probe_unavailable_diagnostics(&self) -> bool {
        self.probe_unavailable_count() > 0
    }

    pub fn item_provenance(&self, index: usize) -> Option<&DeclarationProvenance> {
        self.provenance.get(index)
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn functions(&self) -> impl Iterator<Item = &FunctionBinding> {
        self.items.iter().filter_map(|item| match item {
            BindingItem::Function(function) => Some(function),
            _ => None,
        })
    }

    pub fn records(&self) -> impl Iterator<Item = &RecordBinding> {
        self.items.iter().filter_map(|item| match item {
            BindingItem::Record(record) => Some(record),
            _ => None,
        })
    }

    pub fn enums(&self) -> impl Iterator<Item = &EnumBinding> {
        self.items.iter().filter_map(|item| match item {
            BindingItem::Enum(enum_binding) => Some(enum_binding),
            _ => None,
        })
    }

    pub fn type_aliases(&self) -> impl Iterator<Item = &TypeAliasBinding> {
        self.items.iter().filter_map(|item| match item {
            BindingItem::TypeAlias(type_alias) => Some(type_alias),
            _ => None,
        })
    }

    pub fn variables(&self) -> impl Iterator<Item = &VariableBinding> {
        self.items.iter().filter_map(|item| match item {
            BindingItem::Variable(variable) => Some(variable),
            _ => None,
        })
    }

    pub fn unsupported_items(&self) -> impl Iterator<Item = &UnsupportedItem> {
        self.items.iter().filter_map(|item| match item {
            BindingItem::Unsupported(unsupported) => Some(unsupported),
            _ => None,
        })
    }

    pub fn find_function(&self, name: &str) -> Option<&FunctionBinding> {
        self.functions().find(|item| item.name == name)
    }

    pub fn find_record(&self, name: &str) -> Option<&RecordBinding> {
        self.records()
            .find(|item| item.name.as_deref() == Some(name))
    }

    pub fn find_enum(&self, name: &str) -> Option<&EnumBinding> {
        self.enums().find(|item| item.name.as_deref() == Some(name))
    }

    pub fn find_type_alias(&self, name: &str) -> Option<&TypeAliasBinding> {
        self.type_aliases().find(|item| item.name == name)
    }

    pub fn find_variable(&self, name: &str) -> Option<&VariableBinding> {
        self.variables().find(|item| item.name == name)
    }

    pub fn find_unsupported(&self, name: &str) -> Option<&UnsupportedItem> {
        self.unsupported_items()
            .find(|item| item.name.as_deref() == Some(name))
    }

    pub fn function_count(&self) -> usize {
        self.functions().count()
    }

    pub fn record_count(&self) -> usize {
        self.records().count()
    }

    pub fn enum_count(&self) -> usize {
        self.enums().count()
    }

    pub fn type_alias_count(&self) -> usize {
        self.type_aliases().count()
    }

    pub fn variable_count(&self) -> usize {
        self.variables().count()
    }

    pub fn unsupported_count(&self) -> usize {
        self.unsupported_items().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticKind;

    #[test]
    fn binding_package_default_is_empty() {
        let pkg = BindingPackage::new();
        assert!(pkg.is_empty());
        assert_eq!(pkg.item_count(), 0);
        assert!(!pkg.has_diagnostics());
        assert!(pkg.source_path.is_none());
        assert_eq!(pkg.target, BindingTarget::default());
        assert_eq!(pkg.inputs, BindingInputs::default());
        assert!(pkg.macros.is_empty());
        assert!(pkg.layouts.is_empty());
        assert_eq!(pkg.link, BindingLinkSurface::default());
        assert!(pkg.provenance.is_empty());
        assert!(pkg.macro_provenance.is_empty());
        assert!(pkg.effective_macro_environment.is_empty());
    }

    #[test]
    fn binding_package_query_helpers_report_counts() {
        let mut pkg = BindingPackage::new();
        pkg.macros.push(MacroBinding {
            name: "API_LEVEL".into(),
            body: "7".into(),
            function_like: false,
            form: MacroForm::ObjectLike,
            kind: MacroKind::Integer,
            category: MacroCategory::BindableConstant,
            value: Some(MacroValue::Integer(7)),
        });
        pkg.layouts.push(TypeLayout {
            name: "size_t".into(),
            size: 8,
            align: 8,
        });
        pkg.diagnostics.push(Diagnostic {
            kind: DiagnosticKind::DeclarationUnsupported,
            severity: crate::Severity::Warning,
            message: "unsupported".into(),
            location: None,
            item_name: Some("flags".into()),
            artifact_path: None,
        });
        pkg.items.push(BindingItem::Function(FunctionBinding {
            name: "malloc".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![ParameterBinding {
                name: Some("size".into()),
                ty: BindingType::ULong,
            }],
            return_type: BindingType::ptr(BindingType::Void),
            variadic: false,
            source_offset: Some(1),
        }));
        pkg.items.push(BindingItem::Record(RecordBinding {
            kind: RecordKind::Struct,
            name: Some("point".into()),
            fields: Some(vec![FieldBinding {
                name: Some("x".into()),
                ty: BindingType::Int,
                bit_width: None,
                layout: None,
            }]),
            representation: None,
            abi_confidence: None,
            source_offset: Some(2),
        }));
        pkg.items.push(BindingItem::Enum(EnumBinding {
            name: Some("mode".into()),
            variants: vec![EnumVariant {
                name: "MODE_A".into(),
                value: Some(0),
            }],
            representation: None,
            abi_confidence: None,
            source_offset: Some(3),
        }));
        pkg.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name: "size_t".into(),
            target: BindingType::ULong,
            canonical_resolution: None,
            abi_confidence: None,
            source_offset: Some(4),
        }));
        pkg.items.push(BindingItem::Variable(VariableBinding {
            name: "errno".into(),
            ty: BindingType::Int,
            source_offset: Some(5),
        }));
        pkg.items.push(BindingItem::Unsupported(UnsupportedItem {
            name: Some("flags".into()),
            reason: "bitfield".into(),
            source_offset: Some(6),
        }));

        assert!(!pkg.is_empty());
        assert!(pkg.has_diagnostics());
        assert_eq!(pkg.item_count(), 6);
        assert_eq!(pkg.function_count(), 1);
        assert_eq!(pkg.record_count(), 1);
        assert_eq!(pkg.enum_count(), 1);
        assert_eq!(pkg.type_alias_count(), 1);
        assert_eq!(pkg.variable_count(), 1);
        assert_eq!(pkg.unsupported_count(), 1);
    }

    #[test]
    fn binding_package_typed_iterators_filter_by_kind() {
        let mut pkg = BindingPackage::new();
        pkg.items.push(BindingItem::Function(FunctionBinding {
            name: "malloc".into(),
            calling_convention: CallingConvention::C,
            parameters: Vec::new(),
            return_type: BindingType::ptr(BindingType::Void),
            variadic: false,
            source_offset: Some(1),
        }));
        pkg.items.push(BindingItem::Record(RecordBinding {
            kind: RecordKind::Struct,
            name: Some("point".into()),
            fields: None,
            representation: None,
            abi_confidence: None,
            source_offset: Some(2),
        }));
        pkg.items.push(BindingItem::Enum(EnumBinding {
            name: Some("mode".into()),
            variants: vec![EnumVariant {
                name: "MODE_A".into(),
                value: Some(0),
            }],
            representation: None,
            abi_confidence: None,
            source_offset: Some(3),
        }));
        pkg.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name: "size_t".into(),
            target: BindingType::ULong,
            canonical_resolution: None,
            abi_confidence: None,
            source_offset: Some(4),
        }));
        pkg.items.push(BindingItem::Variable(VariableBinding {
            name: "errno".into(),
            ty: BindingType::Int,
            source_offset: Some(5),
        }));
        pkg.items.push(BindingItem::Unsupported(UnsupportedItem {
            name: Some("flags".into()),
            reason: "bitfield".into(),
            source_offset: Some(6),
        }));

        assert_eq!(
            pkg.functions()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["malloc"]
        );
        assert_eq!(
            pkg.records()
                .map(|item| item.name.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("point")]
        );
        assert_eq!(
            pkg.enums()
                .map(|item| item.name.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("mode")]
        );
        assert_eq!(
            pkg.type_aliases()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["size_t"]
        );
        assert_eq!(
            pkg.variables()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["errno"]
        );
        assert_eq!(
            pkg.unsupported_items()
                .map(|item| item.reason.as_str())
                .collect::<Vec<_>>(),
            vec!["bitfield"]
        );
    }

    #[test]
    fn binding_package_item_provenance_helper_returns_entry() {
        let mut pkg = BindingPackage::new();
        pkg.provenance.push(DeclarationProvenance {
            item_name: Some("demo".into()),
            item_kind: Some(BindingItemKind::Function),
            source_offset: Some(12),
            source_origin: Some(SourceOrigin::Entry),
            source_location: Some(SourceLocation {
                file: "demo.h".into(),
                line: Some(3),
                column: Some(5),
            }),
        });

        let prov = pkg.item_provenance(0).unwrap();
        assert_eq!(prov.item_name.as_deref(), Some("demo"));
        assert_eq!(prov.item_kind, Some(BindingItemKind::Function));
        assert_eq!(prov.source_offset, Some(12));
        assert_eq!(prov.source_origin, Some(SourceOrigin::Entry));
        assert_eq!(prov.source_location.as_ref().unwrap().file, "demo.h");
    }

    #[test]
    fn binding_package_lookup_helpers_find_named_items() {
        let mut pkg = BindingPackage::new();
        pkg.items.push(BindingItem::Function(FunctionBinding {
            name: "malloc".into(),
            calling_convention: CallingConvention::C,
            parameters: Vec::new(),
            return_type: BindingType::ptr(BindingType::Void),
            variadic: false,
            source_offset: Some(1),
        }));
        pkg.items.push(BindingItem::Record(RecordBinding {
            kind: RecordKind::Struct,
            name: Some("point".into()),
            fields: None,
            representation: None,
            abi_confidence: None,
            source_offset: Some(2),
        }));
        pkg.items.push(BindingItem::Enum(EnumBinding {
            name: Some("mode".into()),
            variants: vec![EnumVariant {
                name: "MODE_A".into(),
                value: Some(0),
            }],
            representation: None,
            abi_confidence: None,
            source_offset: Some(3),
        }));
        pkg.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name: "size_t".into(),
            target: BindingType::ULong,
            canonical_resolution: None,
            abi_confidence: None,
            source_offset: Some(4),
        }));
        pkg.items.push(BindingItem::Variable(VariableBinding {
            name: "errno".into(),
            ty: BindingType::Int,
            source_offset: Some(5),
        }));
        pkg.items.push(BindingItem::Unsupported(UnsupportedItem {
            name: Some("flags".into()),
            reason: "bitfield".into(),
            source_offset: Some(6),
        }));

        assert_eq!(
            pkg.find_function("malloc").map(|item| item.name.as_str()),
            Some("malloc")
        );
        assert_eq!(
            pkg.find_record("point")
                .and_then(|item| item.name.as_deref()),
            Some("point")
        );
        assert_eq!(
            pkg.find_enum("mode").and_then(|item| item.name.as_deref()),
            Some("mode")
        );
        assert_eq!(
            pkg.find_type_alias("size_t").map(|item| item.name.as_str()),
            Some("size_t")
        );
        assert_eq!(
            pkg.find_variable("errno").map(|item| item.name.as_str()),
            Some("errno")
        );
        assert_eq!(
            pkg.find_unsupported("flags")
                .and_then(|item| item.name.as_deref()),
            Some("flags")
        );

        assert!(pkg.find_function("calloc").is_none());
        assert!(pkg.find_record("vector").is_none());
        assert!(pkg.find_enum("MODE_B").is_none());
        assert!(pkg.find_type_alias("ssize_t").is_none());
        assert!(pkg.find_variable("stdin").is_none());
        assert!(pkg.find_unsupported("padding").is_none());
    }

    #[test]
    fn binding_type_pointer_nesting() {
        let ty = BindingType::ptr(BindingType::ptr(BindingType::Char));
        match &ty {
            BindingType::Pointer { pointee: inner, .. } => match inner.as_ref() {
                BindingType::Pointer {
                    pointee: inner2, ..
                } => {
                    assert_eq!(*inner2.as_ref(), BindingType::Char)
                }
                _ => panic!("expected pointer"),
            },
            _ => panic!("expected pointer"),
        }
    }

    #[test]
    fn opaque_record() {
        let rec = RecordBinding {
            kind: RecordKind::Struct,
            name: Some("FILE".into()),
            fields: None,
            representation: None,
            abi_confidence: None,
            source_offset: None,
        };
        assert!(rec.is_opaque());
    }

    #[test]
    fn record_with_fields() {
        let rec = RecordBinding {
            kind: RecordKind::Struct,
            name: Some("point".into()),
            fields: Some(vec![
                FieldBinding {
                    name: Some("x".into()),
                    ty: BindingType::Int,
                    bit_width: None,
                    layout: None,
                },
                FieldBinding {
                    name: Some("y".into()),
                    ty: BindingType::Int,
                    bit_width: None,
                    layout: None,
                },
            ]),
            representation: None,
            abi_confidence: None,
            source_offset: None,
        };
        assert!(!rec.is_opaque());
        assert_eq!(rec.fields.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn record_representation_roundtrip() {
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
    fn field_binding_tracks_partial_bitfield_metadata() {
        let field = FieldBinding {
            name: Some("flags".into()),
            ty: BindingType::UInt,
            bit_width: Some(3),
            layout: None,
        };
        assert!(field.is_bitfield());
    }

    #[test]
    fn function_binding_construction() {
        let func = FunctionBinding {
            name: "printf".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![ParameterBinding {
                name: Some("fmt".into()),
                ty: BindingType::ptr(BindingType::Char),
            }],
            return_type: BindingType::Int,
            variadic: true,
            source_offset: None,
        };
        assert_eq!(func.name, "printf");
        assert!(func.variadic);
    }

    #[test]
    fn ir_serialization_roundtrip() {
        let mut pkg = BindingPackage::new();
        pkg.source_path = Some("test.h".into());
        pkg.target = BindingTarget {
            target_triple: Some("x86_64-unknown-linux-gnu".into()),
            compiler_command: Some("gcc".into()),
            compiler_version: Some("gcc (GCC) 13.2.0".into()),
            flavor: Some("gnu-c11".into()),
        };
        pkg.inputs = BindingInputs {
            entry_headers: vec!["test.h".into()],
            include_dirs: vec!["/usr/include".into()],
            defines: vec![BindingDefine {
                name: "DEBUG".into(),
                value: Some("1".into()),
            }],
        };
        pkg.macros = vec![MacroBinding {
            name: "API_LEVEL".into(),
            body: "7".into(),
            function_like: false,
            form: MacroForm::ObjectLike,
            kind: MacroKind::Integer,
            category: MacroCategory::BindableConstant,
            value: Some(MacroValue::Integer(7)),
        }];
        pkg.layouts = vec![TypeLayout {
            name: "size_t".into(),
            size: 8,
            align: 8,
        }];
        pkg.link = BindingLinkSurface {
            preferred_mode: LinkResolutionMode::PreferDynamic,
            native_surface_kind: NativeSurfaceKind::Mixed,
            platform_constraints: vec!["macos".into()],
            include_paths: vec!["/usr/include".into()],
            framework_paths: vec!["/System/Library/Frameworks".into()],
            library_paths: vec!["/usr/lib".into()],
            libraries: vec![LinkLibrary {
                name: "z".into(),
                kind: LinkLibraryKind::Dynamic,
                source: LinkRequirementSource::Declared,
            }],
            frameworks: vec![LinkFramework {
                name: "CoreFoundation".into(),
                source: LinkRequirementSource::Inferred,
            }],
            artifacts: vec![LinkArtifact {
                path: "/usr/lib/libz.so".into(),
                kind: LinkArtifactKind::SharedLibrary,
                source: LinkRequirementSource::Discovered,
            }],
            ordered_inputs: vec![
                LinkInput::Framework(LinkFramework {
                    name: "CoreFoundation".into(),
                    source: LinkRequirementSource::Inferred,
                }),
                LinkInput::Library(LinkLibrary {
                    name: "z".into(),
                    kind: LinkLibraryKind::Dynamic,
                    source: LinkRequirementSource::Declared,
                }),
                LinkInput::Artifact(LinkArtifact {
                    path: "/usr/lib/libz.so".into(),
                    kind: LinkArtifactKind::SharedLibrary,
                    source: LinkRequirementSource::Discovered,
                }),
            ],
        };
        pkg.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name: "size_t".into(),
            target: BindingType::ULong,
            canonical_resolution: None,
            abi_confidence: None,
            source_offset: Some(0),
        }));
        pkg.items.push(BindingItem::Function(FunctionBinding {
            name: "malloc".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![ParameterBinding {
                name: Some("size".into()),
                ty: BindingType::TypedefRef("size_t".into()),
            }],
            return_type: BindingType::ptr(BindingType::Void),
            variadic: false,
            source_offset: Some(100),
        }));

        let json = serde_json::to_string_pretty(&pkg).unwrap();
        let pkg2: BindingPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg, pkg2);
    }

    #[test]
    fn binding_package_defaults_on_old_json() {
        let json = r#"{
            "schema_version": 1,
            "linc_version": "0.1.0",
            "source_path": "legacy.h",
            "items": [],
            "diagnostics": []
        }"#;
        let pkg: BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.target, BindingTarget::default());
        assert_eq!(pkg.inputs, BindingInputs::default());
        assert!(pkg.macros.is_empty());
        assert!(pkg.layouts.is_empty());
        assert_eq!(pkg.link, BindingLinkSurface::default());
    }

    #[test]
    fn binding_package_accepts_empty_nested_metadata_objects() {
        let json = r#"{
            "schema_version": 1,
            "linc_version": "0.1.0",
            "target": {},
            "inputs": {},
            "link": {},
            "source_path": "legacy.h",
            "items": [],
            "diagnostics": []
        }"#;
        let pkg: BindingPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.target, BindingTarget::default());
        assert_eq!(pkg.inputs, BindingInputs::default());
        assert!(pkg.macros.is_empty());
        assert!(pkg.layouts.is_empty());
        assert_eq!(pkg.link, BindingLinkSurface::default());
    }

    #[test]
    fn macro_binding_serialization_roundtrip() {
        let macros = vec![
            MacroBinding {
                name: "API_LEVEL".into(),
                body: "7".into(),
                function_like: false,
                form: MacroForm::ObjectLike,
                kind: MacroKind::Integer,
                category: MacroCategory::BindableConstant,
                value: Some(MacroValue::Integer(7)),
            },
            MacroBinding {
                name: "LOG".into(),
                body: "fmt".into(),
                function_like: true,
                form: MacroForm::FunctionLike,
                kind: MacroKind::Other,
                category: MacroCategory::Unsupported,
                value: None,
            },
        ];
        let json = serde_json::to_string(&macros).unwrap();
        let decoded: Vec<MacroBinding> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, macros);
    }

    #[test]
    fn macro_binding_defaults_category_on_old_json() {
        let json = r#"[
            {
                "name": "API_LEVEL",
                "body": "7",
                "function_like": false,
                "kind": "Integer"
            }
        ]"#;
        let decoded: Vec<MacroBinding> = serde_json::from_str(json).unwrap();
        assert_eq!(decoded[0].category, MacroCategory::BindableConstant);
        assert_eq!(decoded[0].form, MacroForm::ObjectLike);
        assert_eq!(decoded[0].value, None);
    }

    #[test]
    fn macro_binding_distinguishes_unsupported_forms() {
        let function_like = MacroBinding {
            name: "LOG".into(),
            body: "fmt".into(),
            function_like: true,
            form: MacroForm::FunctionLike,
            kind: MacroKind::Other,
            category: MacroCategory::Unsupported,
            value: None,
        };
        let object_like = MacroBinding {
            name: "INTERNAL_SENTINEL".into(),
            body: "((void*)0)".into(),
            function_like: false,
            form: MacroForm::ObjectLike,
            kind: MacroKind::Other,
            category: MacroCategory::Unsupported,
            value: None,
        };

        assert!(function_like.is_unsupported_function_like());
        assert!(!function_like.is_unsupported_object_like());
        assert!(object_like.is_unsupported_object_like());
        assert!(!object_like.is_unsupported_function_like());
    }

    #[test]
    fn type_layout_serialization_roundtrip() {
        let layouts = vec![TypeLayout {
            name: "struct widget".into(),
            size: 16,
            align: 8,
        }];
        let json = serde_json::to_string(&layouts).unwrap();
        let decoded: Vec<TypeLayout> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, layouts);
    }

    #[test]
    fn enum_binding_construction() {
        let e = EnumBinding {
            name: Some("color".into()),
            variants: vec![
                EnumVariant {
                    name: "RED".into(),
                    value: Some(0),
                },
                EnumVariant {
                    name: "GREEN".into(),
                    value: Some(1),
                },
                EnumVariant {
                    name: "BLUE".into(),
                    value: Some(2),
                },
            ],
            representation: None,
            abi_confidence: None,
            source_offset: None,
        };
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[0].value, Some(0));
    }

    #[test]
    fn enum_representation_roundtrip() {
        let representation = EnumRepresentation {
            underlying_size: Some(4),
            is_signed: Some(true),
        };
        let json = serde_json::to_string(&representation).unwrap();
        let decoded: EnumRepresentation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, representation);
    }

    #[test]
    fn abi_confidence_roundtrip() {
        let json = serde_json::to_string(&AbiConfidence::PartialBitfieldLayout).unwrap();
        let decoded: AbiConfidence = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, AbiConfidence::PartialBitfieldLayout);
    }

    #[test]
    fn abi_confidence_summary_helpers_distinguish_exact_and_partial_layouts() {
        assert!(!AbiConfidence::DeclaredOnly.has_layout_evidence());
        assert_eq!(AbiConfidence::DeclaredOnly.summary_label(), "declared-only");

        assert!(AbiConfidence::LayoutProbed.has_layout_evidence());
        assert!(AbiConfidence::LayoutProbed.is_exact_layout_family());
        assert_eq!(AbiConfidence::LayoutProbed.summary_label(), "layout-probed");

        assert!(AbiConfidence::FieldOffsetsProbed.is_exact_layout_family());
        assert_eq!(
            AbiConfidence::FieldOffsetsProbed.summary_label(),
            "field-offsets-probed"
        );

        assert!(AbiConfidence::RepresentationProbed.is_exact_layout_family());
        assert_eq!(
            AbiConfidence::RepresentationProbed.summary_label(),
            "representation-probed"
        );

        assert!(AbiConfidence::PartialBitfieldLayout.has_layout_evidence());
        assert!(AbiConfidence::PartialBitfieldLayout.is_partial_layout());
        assert!(!AbiConfidence::PartialBitfieldLayout.is_exact_layout_family());
        assert_eq!(
            AbiConfidence::PartialBitfieldLayout.summary_label(),
            "partial-bitfield-layout"
        );
    }

    #[test]
    fn alias_resolution_roundtrip() {
        let resolution = AliasResolution {
            alias_chain: vec!["size_t".into()],
            terminal_target: BindingType::ULong,
        };
        let json = serde_json::to_string(&resolution).unwrap();
        let decoded: AliasResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, resolution);
    }

    #[test]
    fn function_pointer_type() {
        let ty = BindingType::FunctionPointer {
            return_type: Box::new(BindingType::Void),
            parameters: vec![BindingType::Int, BindingType::ptr(BindingType::Void)],
            variadic: false,
        };
        match &ty {
            BindingType::FunctionPointer {
                parameters,
                variadic,
                ..
            } => {
                assert_eq!(parameters.len(), 2);
                assert!(!variadic);
            }
            _ => panic!("expected function pointer"),
        }
    }

    #[test]
    fn const_pointer_vs_mut_pointer() {
        let const_ptr = BindingType::const_ptr(BindingType::Char);
        let mut_ptr = BindingType::ptr(BindingType::Char);
        assert_ne!(const_ptr, mut_ptr);

        match &const_ptr {
            BindingType::Pointer { const_pointee, .. } => assert!(const_pointee),
            _ => panic!("expected pointer"),
        }
        match &mut_ptr {
            BindingType::Pointer { const_pointee, .. } => assert!(!const_pointee),
            _ => panic!("expected pointer"),
        }
    }

    #[test]
    fn const_pointer_serialization_roundtrip() {
        let ty = BindingType::const_ptr(BindingType::Void);
        let json = serde_json::to_string(&ty).unwrap();
        let ty2: BindingType = serde_json::from_str(&json).unwrap();
        assert_eq!(ty, ty2);
        match &ty2 {
            BindingType::Pointer { const_pointee, .. } => assert!(const_pointee),
            _ => panic!("expected pointer"),
        }
    }

    #[test]
    fn link_library_serialization_roundtrip() {
        let link = BindingLinkSurface {
            preferred_mode: LinkResolutionMode::PreferStatic,
            native_surface_kind: NativeSurfaceKind::Mixed,
            platform_constraints: vec!["linux".into(), "x86_64".into()],
            include_paths: vec!["include".into()],
            framework_paths: vec!["frameworks".into()],
            library_paths: vec!["lib".into()],
            libraries: vec![
                LinkLibrary {
                    name: "ssl".into(),
                    kind: LinkLibraryKind::Default,
                    source: LinkRequirementSource::Declared,
                },
                LinkLibrary {
                    name: "crypto".into(),
                    kind: LinkLibraryKind::Static,
                    source: LinkRequirementSource::Inferred,
                },
            ],
            frameworks: vec![LinkFramework {
                name: "Security".into(),
                source: LinkRequirementSource::Declared,
            }],
            artifacts: vec![
                LinkArtifact {
                    path: "libssl.a".into(),
                    kind: LinkArtifactKind::StaticLibrary,
                    source: LinkRequirementSource::Discovered,
                },
                LinkArtifact {
                    path: "plugin.o".into(),
                    kind: LinkArtifactKind::Object,
                    source: LinkRequirementSource::Declared,
                },
            ],
            ordered_inputs: vec![
                LinkInput::Framework(LinkFramework {
                    name: "Security".into(),
                    source: LinkRequirementSource::Declared,
                }),
                LinkInput::Library(LinkLibrary {
                    name: "ssl".into(),
                    kind: LinkLibraryKind::Default,
                    source: LinkRequirementSource::Declared,
                }),
                LinkInput::Library(LinkLibrary {
                    name: "crypto".into(),
                    kind: LinkLibraryKind::Static,
                    source: LinkRequirementSource::Inferred,
                }),
                LinkInput::Artifact(LinkArtifact {
                    path: "libssl.a".into(),
                    kind: LinkArtifactKind::StaticLibrary,
                    source: LinkRequirementSource::Discovered,
                }),
                LinkInput::Artifact(LinkArtifact {
                    path: "plugin.o".into(),
                    kind: LinkArtifactKind::Object,
                    source: LinkRequirementSource::Declared,
                }),
            ],
        };
        let json = serde_json::to_string(&link).unwrap();
        let decoded: BindingLinkSurface = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, link);
    }

    #[test]
    fn binding_link_surface_defaults_on_old_json() {
        let json = r#"{
            "include_paths": ["include"],
            "library_paths": ["lib"],
            "libraries": [
                { "name": "ssl", "kind": "Dynamic" }
            ],
            "artifacts": [
                { "path": "native/libssl.so", "kind": "SharedLibrary" }
            ]
        }"#;
        let decoded: BindingLinkSurface = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.preferred_mode, LinkResolutionMode::Default);
        assert_eq!(decoded.native_surface_kind, NativeSurfaceKind::HeaderOnly);
        assert!(decoded.platform_constraints.is_empty());
        assert_eq!(decoded.libraries[0].source, LinkRequirementSource::Declared);
        assert_eq!(decoded.artifacts[0].source, LinkRequirementSource::Declared);
        assert!(decoded.framework_paths.is_empty());
        assert!(decoded.frameworks.is_empty());
        assert!(decoded.ordered_inputs.is_empty());
    }

    #[test]
    fn binding_package_serialization_is_deterministic() {
        let mut pkg = BindingPackage::new();
        pkg.items.push(BindingItem::Function(FunctionBinding {
            name: "init".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![ParameterBinding {
                name: Some("flags".into()),
                ty: BindingType::UInt,
            }],
            return_type: BindingType::Int,
            variadic: false,
            source_offset: None,
        }));
        pkg.items.push(BindingItem::Variable(VariableBinding {
            name: "version".into(),
            ty: BindingType::Int,
            source_offset: None,
        }));
        pkg.macros.push(MacroBinding {
            name: "VERSION".into(),
            body: "1".into(),
            function_like: false,
            form: MacroForm::ObjectLike,
            kind: MacroKind::Integer,
            category: MacroCategory::BindableConstant,
            value: Some(MacroValue::Integer(1)),
        });

        let json1 = serde_json::to_string_pretty(&pkg).unwrap();
        let json2 = serde_json::to_string_pretty(&pkg).unwrap();
        assert_eq!(json1, json2, "serialization must be deterministic");

        // Roundtrip preserves equality
        let restored: BindingPackage = serde_json::from_str(&json1).unwrap();
        let json3 = serde_json::to_string_pretty(&restored).unwrap();
        assert_eq!(json1, json3, "roundtrip must preserve deterministic output");
    }
}
