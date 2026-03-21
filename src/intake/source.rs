//! Frontend-neutral source contract types.
//!
//! These types represent the normalized declarations, macros, and link
//! requirements that a frontend produces. LINC core logic consumes these
//! types instead of parser-specific AST structures.

use serde::{Deserialize, Serialize};

/// A normalized source package produced by a frontend.
///
/// This is the primary intake contract for LINC. A frontend
/// populates this after scanning and extracting source-level information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SourcePackage {
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub declarations: Vec<SourceDeclaration>,
    #[serde(default)]
    pub macros: Vec<SourceMacro>,
    #[serde(default)]
    pub link_requirements: Vec<SourceLinkRequirement>,
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub entry_headers: Vec<String>,
    #[serde(default)]
    pub defines: Vec<(String, Option<String>)>,
    #[serde(default)]
    pub target_triple: Option<String>,
    #[serde(default)]
    pub compiler_command: Option<String>,
    #[serde(default)]
    pub compiler_version: Option<String>,
}

/// A single extracted declaration from source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceDeclaration {
    Function(SourceFunction),
    Record(SourceRecord),
    Enum(SourceEnum),
    TypeAlias(SourceTypeAlias),
    Variable(SourceVariable),
}

/// An extracted function declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFunction {
    pub name: String,
    #[serde(default)]
    pub parameters: Vec<SourceParameter>,
    pub return_type: SourceType,
    #[serde(default)]
    pub variadic: bool,
    #[serde(default)]
    pub source_offset: Option<usize>,
}

/// A single function parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceParameter {
    pub name: Option<String>,
    pub ty: SourceType,
}

/// An extracted record (struct or union) declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub name: Option<String>,
    #[serde(default)]
    pub is_union: bool,
    /// `None` means opaque/forward declaration.
    pub fields: Option<Vec<SourceField>>,
    #[serde(default)]
    pub source_offset: Option<usize>,
}

/// A single record field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceField {
    pub name: Option<String>,
    pub ty: SourceType,
    #[serde(default)]
    pub bit_width: Option<u64>,
}

/// An extracted enum declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceEnum {
    pub name: Option<String>,
    #[serde(default)]
    pub variants: Vec<SourceEnumVariant>,
    #[serde(default)]
    pub source_offset: Option<usize>,
}

/// A single enum constant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceEnumVariant {
    pub name: String,
    pub value: Option<i128>,
}

/// An extracted typedef or alias declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceTypeAlias {
    pub name: String,
    pub target: SourceType,
    #[serde(default)]
    pub source_offset: Option<usize>,
}

/// An extracted external variable declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceVariable {
    pub name: String,
    pub ty: SourceType,
    #[serde(default)]
    pub source_offset: Option<usize>,
}

/// A captured preprocessor macro from source scanning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMacro {
    pub name: String,
    pub body: String,
    #[serde(default)]
    pub function_like: bool,
}

/// A link requirement declared or inferred from source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLinkRequirement {
    pub name: String,
    #[serde(default)]
    pub kind: SourceLinkKind,
}

/// Kind of link requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SourceLinkKind {
    #[default]
    Library,
    StaticLibrary,
    DynamicLibrary,
    Framework,
}

/// Frontend-neutral type representation.
///
/// This is a simplified, language-neutral type model that captures what LINC
/// needs for link analysis, validation, and ABI probing. It intentionally
/// does not try to be a fully lossless C type system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceType {
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
    Pointer(Box<SourceType>),
    ConstPointer(Box<SourceType>),
    Array(Box<SourceType>, Option<u64>),
    FunctionPointer {
        return_type: Box<SourceType>,
        parameters: Vec<SourceType>,
        variadic: bool,
    },
    TypedefRef(String),
    RecordRef(String),
    EnumRef(String),
    Opaque(String),
    Const(Box<SourceType>),
    Volatile(Box<SourceType>),
}
