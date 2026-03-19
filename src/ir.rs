use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingPackage {
    pub source_path: Option<String>,
    pub items: Vec<BindingItem>,
    pub diagnostics: Vec<Diagnostic>,
}

impl BindingPackage {
    pub fn new() -> Self {
        Self {
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
}
