//! Declaration and type representation types for the LINC IR.
//!
//! These types model extracted C declarations and their type system in a
//! language-neutral binding representation.

use serde::{Deserialize, Serialize};

/// One extracted declaration or unsupported declaration placeholder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BindingItem {
    Function(FunctionBinding),
    Record(RecordBinding),
    Enum(EnumBinding),
    TypeAlias(TypeAliasBinding),
    Variable(VariableBinding),
    Unsupported(UnsupportedItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingItemKind {
    Function,
    Record,
    Enum,
    TypeAlias,
    Variable,
    Unsupported,
}

/// Type representation used by the extracted IR.
///
/// Invariant: this is a language-neutral binding model, not a fully lossless C type system.
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
        #[serde(default)]
        qualifiers: TypeQualifiers,
    },
    Array(Box<BindingType>, Option<u64>),
    Qualified {
        ty: Box<BindingType>,
        #[serde(default)]
        qualifiers: TypeQualifiers,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TypeQualifiers {
    #[serde(default)]
    pub is_const: bool,
    #[serde(default)]
    pub is_volatile: bool,
    #[serde(default)]
    pub is_restrict: bool,
    #[serde(default)]
    pub is_atomic: bool,
}

impl BindingType {
    pub fn ptr(pointee: BindingType) -> Self {
        BindingType::Pointer {
            pointee: Box::new(pointee),
            const_pointee: false,
            qualifiers: TypeQualifiers::default(),
        }
    }

    pub fn const_ptr(pointee: BindingType) -> Self {
        BindingType::Pointer {
            pointee: Box::new(pointee),
            const_pointee: true,
            qualifiers: TypeQualifiers::default(),
        }
    }

    pub fn qualified(ty: BindingType, qualifiers: TypeQualifiers) -> Self {
        if qualifiers == TypeQualifiers::default() {
            ty
        } else {
            BindingType::Qualified {
                ty: Box::new(ty),
                qualifiers,
            }
        }
    }

    pub fn is_void(&self) -> bool {
        match self {
            BindingType::Void => true,
            BindingType::Qualified { ty, .. } => ty.is_void(),
            _ => false,
        }
    }
}

/// Calling convention attached to an extracted function declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallingConvention {
    C,
    Cdecl,
    Stdcall,
    Fastcall,
    Vectorcall,
    Thiscall,
    Unknown(String),
}

/// Extracted function declaration.
///
/// Invariant: `name` is always present and `source_offset` is best-effort provenance rather than a
/// standalone source location contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionBinding {
    pub name: String,
    pub calling_convention: CallingConvention,
    pub parameters: Vec<ParameterBinding>,
    pub return_type: BindingType,
    pub variadic: bool,
    pub source_offset: Option<usize>,
}

/// One function parameter.
///
/// Invariant: unnamed parameters are represented with `name == None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterBinding {
    pub name: Option<String>,
    pub ty: BindingType,
}

/// Kind of record declaration represented by `RecordBinding`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordKind {
    Struct,
    Union,
}

/// One field inside a non-opaque record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBinding {
    pub name: Option<String>,
    pub ty: BindingType,
    #[serde(default)]
    pub bit_width: Option<u64>,
    #[serde(default)]
    pub layout: Option<FieldLayout>,
}

impl FieldBinding {
    pub fn is_bitfield(&self) -> bool {
        self.bit_width.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldLayout {
    #[serde(default)]
    pub offset_bytes: Option<u64>,
}

/// Extracted record declaration.
///
/// Invariant: `fields == None` means the record is opaque or otherwise field-incomplete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordBinding {
    pub kind: RecordKind,
    pub name: Option<String>,
    pub fields: Option<Vec<FieldBinding>>,
    #[serde(default)]
    pub representation: Option<RecordRepresentation>,
    #[serde(default)]
    pub abi_confidence: Option<AbiConfidence>,
    pub source_offset: Option<usize>,
}

impl RecordBinding {
    pub fn is_opaque(&self) -> bool {
        self.fields.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRepresentation {
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub align: Option<u64>,
    #[serde(default)]
    pub completeness: Option<String>,
}

/// One enum constant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i128>,
}

/// Extracted enum declaration.
///
/// Invariant: anonymous enums are represented with `name == None`, while `variants` preserves
/// declaration order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumBinding {
    pub name: Option<String>,
    pub variants: Vec<EnumVariant>,
    #[serde(default)]
    pub representation: Option<EnumRepresentation>,
    #[serde(default)]
    pub abi_confidence: Option<AbiConfidence>,
    pub source_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumRepresentation {
    #[serde(default)]
    pub underlying_size: Option<u64>,
    #[serde(default)]
    pub is_signed: Option<bool>,
}

/// Extracted typedef or alias declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeAliasBinding {
    pub name: String,
    pub target: BindingType,
    #[serde(default)]
    pub canonical_resolution: Option<AliasResolution>,
    #[serde(default)]
    pub abi_confidence: Option<AbiConfidence>,
    pub source_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AliasResolution {
    #[serde(default)]
    pub alias_chain: Vec<String>,
    pub terminal_target: BindingType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbiConfidence {
    DeclaredOnly,
    LayoutProbed,
    FieldOffsetsProbed,
    RepresentationProbed,
    PartialBitfieldLayout,
}

/// Extracted external variable declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableBinding {
    pub name: String,
    pub ty: BindingType,
    pub source_offset: Option<usize>,
}

/// Placeholder for a declaration that LINC recognized but could not model directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedItem {
    pub name: Option<String>,
    pub reason: String,
    pub source_offset: Option<usize>,
}

/// Compiler-probed layout evidence for a named type.
///
/// Invariant: `name` is the consumer-visible identity key and `size`/`align` are only present when
/// probing succeeded for that exact subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeLayout {
    pub name: String,
    pub size: u64,
    pub align: u64,
}
