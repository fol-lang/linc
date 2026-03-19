use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;

pub const SCHEMA_VERSION: u32 = 1;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingDefine {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BindingInputs {
    #[serde(default)]
    pub entry_headers: Vec<String>,
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub defines: Vec<BindingDefine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroKind {
    Integer,
    String,
    Expression,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MacroCategory {
    #[default]
    BindableConstant,
    ConfigurationFlag,
    AbiAffecting,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroBinding {
    pub name: String,
    pub body: String,
    pub function_like: bool,
    pub kind: MacroKind,
    #[serde(default)]
    pub category: MacroCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeLayout {
    pub name: String,
    pub size: u64,
    pub align: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkLibraryKind {
    Default,
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LinkResolutionMode {
    #[default]
    Default,
    PreferStatic,
    PreferDynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LinkRequirementSource {
    #[default]
    Declared,
    Inferred,
    Discovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkLibrary {
    pub name: String,
    pub kind: LinkLibraryKind,
    #[serde(default)]
    pub source: LinkRequirementSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkArtifactKind {
    Object,
    StaticLibrary,
    SharedLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkArtifact {
    pub path: String,
    pub kind: LinkArtifactKind,
    #[serde(default)]
    pub source: LinkRequirementSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkInput {
    Library(LinkLibrary),
    Artifact(LinkArtifact),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BindingLinkSurface {
    #[serde(default)]
    pub preferred_mode: LinkResolutionMode,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub library_paths: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<LinkLibrary>,
    #[serde(default)]
    pub artifacts: Vec<LinkArtifact>,
    #[serde(default)]
    pub ordered_inputs: Vec<LinkInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingPackage {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_bic_version")]
    pub bic_version: String,
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
    pub source_path: Option<String>,
    pub items: Vec<BindingItem>,
    pub diagnostics: Vec<Diagnostic>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

fn default_bic_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

impl BindingPackage {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            bic_version: env!("CARGO_PKG_VERSION").to_string(),
            target: BindingTarget::default(),
            inputs: BindingInputs::default(),
            macros: Vec::new(),
            layouts: Vec::new(),
            link: BindingLinkSurface::default(),
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
        self.items.retain(|item| {
            let offset = match item {
                BindingItem::Function(f) => f.source_offset,
                BindingItem::Record(r) => r.source_offset,
                BindingItem::Enum(e) => e.source_offset,
                BindingItem::TypeAlias(t) => t.source_offset,
                BindingItem::Variable(v) => v.source_offset,
                BindingItem::Unsupported(u) => u.source_offset,
            };
            match offset {
                Some(off) => filter.accepts(&origin_map.origin_at(off)),
                None => true, // Keep items without offsets
            }
        });
    }

    pub fn diagnostics_count_by_kind(&self) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        for d in &self.diagnostics {
            let key = format!("{:?}", d.kind);
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BindingItem {
    Function(FunctionBinding),
    Record(RecordBinding),
    Enum(EnumBinding),
    TypeAlias(TypeAliasBinding),
    Variable(VariableBinding),
    Unsupported(UnsupportedItem),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BindingType {
    Void,
    Bool,
    Char,
    SChar,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    ULong,
    LongLong,
    ULongLong,
    Float,
    Double,
    LongDouble,
    Pointer {
        pointee: Box<BindingType>,
        const_pointee: bool,
    },
    Array(Box<BindingType>, Option<u64>),
    FunctionPointer {
        return_type: Box<BindingType>,
        parameters: Vec<BindingType>,
        variadic: bool,
    },
    TypedefRef(String),
    RecordRef(String),
    EnumRef(String),
    Opaque(String),
}

impl BindingType {
    pub fn ptr(pointee: BindingType) -> Self {
        BindingType::Pointer {
            pointee: Box::new(pointee),
            const_pointee: false,
        }
    }

    pub fn const_ptr(pointee: BindingType) -> Self {
        BindingType::Pointer {
            pointee: Box::new(pointee),
            const_pointee: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallingConvention {
    C,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionBinding {
    pub name: String,
    pub calling_convention: CallingConvention,
    pub parameters: Vec<ParameterBinding>,
    pub return_type: BindingType,
    pub variadic: bool,
    pub source_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterBinding {
    pub name: Option<String>,
    pub ty: BindingType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordKind {
    Struct,
    Union,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBinding {
    pub name: Option<String>,
    pub ty: BindingType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordBinding {
    pub kind: RecordKind,
    pub name: Option<String>,
    pub fields: Option<Vec<FieldBinding>>,
    pub source_offset: Option<usize>,
}

impl RecordBinding {
    pub fn is_opaque(&self) -> bool {
        self.fields.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i128>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumBinding {
    pub name: Option<String>,
    pub variants: Vec<EnumVariant>,
    pub source_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeAliasBinding {
    pub name: String,
    pub target: BindingType,
    pub source_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableBinding {
    pub name: String,
    pub ty: BindingType,
    pub source_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedItem {
    pub name: Option<String>,
    pub reason: String,
    pub source_offset: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_package_default_is_empty() {
        let pkg = BindingPackage::new();
        assert!(pkg.items.is_empty());
        assert!(pkg.diagnostics.is_empty());
        assert!(pkg.source_path.is_none());
        assert_eq!(pkg.target, BindingTarget::default());
        assert_eq!(pkg.inputs, BindingInputs::default());
        assert!(pkg.macros.is_empty());
        assert!(pkg.layouts.is_empty());
        assert_eq!(pkg.link, BindingLinkSurface::default());
    }

    #[test]
    fn binding_type_pointer_nesting() {
        let ty = BindingType::ptr(BindingType::ptr(BindingType::Char));
        match &ty {
            BindingType::Pointer { pointee: inner, .. } => match inner.as_ref() {
                BindingType::Pointer { pointee: inner2, .. } => {
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
                FieldBinding { name: Some("x".into()), ty: BindingType::Int },
                FieldBinding { name: Some("y".into()), ty: BindingType::Int },
            ]),
            source_offset: None,
        };
        assert!(!rec.is_opaque());
        assert_eq!(rec.fields.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn function_binding_construction() {
        let func = FunctionBinding {
            name: "printf".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![
                ParameterBinding {
                    name: Some("fmt".into()),
                    ty: BindingType::ptr(BindingType::Char),
                },
            ],
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
            kind: MacroKind::Integer,
            category: MacroCategory::BindableConstant,
        }];
        pkg.layouts = vec![TypeLayout {
            name: "size_t".into(),
            size: 8,
            align: 8,
        }];
        pkg.link = BindingLinkSurface {
            preferred_mode: LinkResolutionMode::PreferDynamic,
            include_paths: vec!["/usr/include".into()],
            library_paths: vec!["/usr/lib".into()],
            libraries: vec![LinkLibrary {
                name: "z".into(),
                kind: LinkLibraryKind::Dynamic,
                source: LinkRequirementSource::Declared,
            }],
            artifacts: vec![LinkArtifact {
                path: "/usr/lib/libz.so".into(),
                kind: LinkArtifactKind::SharedLibrary,
                source: LinkRequirementSource::Discovered,
            }],
            ordered_inputs: vec![
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
            "bic_version": "0.1.0",
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
            "bic_version": "0.1.0",
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
                kind: MacroKind::Integer,
                category: MacroCategory::BindableConstant,
            },
            MacroBinding {
                name: "LOG".into(),
                body: "fmt".into(),
                function_like: true,
                kind: MacroKind::Other,
                category: MacroCategory::Unsupported,
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
                EnumVariant { name: "RED".into(), value: Some(0) },
                EnumVariant { name: "GREEN".into(), value: Some(1) },
                EnumVariant { name: "BLUE".into(), value: Some(2) },
            ],
            source_offset: None,
        };
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[0].value, Some(0));
    }

    #[test]
    fn function_pointer_type() {
        let ty = BindingType::FunctionPointer {
            return_type: Box::new(BindingType::Void),
            parameters: vec![BindingType::Int, BindingType::ptr(BindingType::Void)],
            variadic: false,
        };
        match &ty {
            BindingType::FunctionPointer { parameters, variadic, .. } => {
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
            include_paths: vec!["include".into()],
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
        assert_eq!(decoded.libraries[0].source, LinkRequirementSource::Declared);
        assert_eq!(decoded.artifacts[0].source, LinkRequirementSource::Declared);
        assert!(decoded.ordered_inputs.is_empty());
    }
}
