//! Macro-related types for the LINC IR.
//!
//! These types represent captured preprocessor macros, their classification,
//! and their provenance.

use serde::{Deserialize, Serialize};

use crate::line_markers::{SourceLocation, SourceOrigin};

/// High-level interpretation of a captured macro body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroKind {
    Integer,
    String,
    Expression,
    Other,
}

/// Whether a macro is object-like or function-like at the preprocessor level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MacroForm {
    #[default]
    ObjectLike,
    FunctionLike,
}

/// Stable consumer-facing classification for how a macro should be treated downstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MacroCategory {
    #[default]
    BindableConstant,
    ConfigurationFlag,
    AbiAffecting,
    Unsupported,
}

/// Parsed constant value for macros that are safe to lower directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroValue {
    Integer(i128),
    String(String),
}

/// One captured preprocessor macro.
///
/// Invariant: `function_like` is preserved for compatibility, while `form` is the preferred
/// normalized representation for new consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroBinding {
    pub name: String,
    pub body: String,
    pub function_like: bool,
    #[serde(default)]
    pub form: MacroForm,
    pub kind: MacroKind,
    #[serde(default)]
    pub category: MacroCategory,
    #[serde(default)]
    pub value: Option<MacroValue>,
}

impl MacroBinding {
    pub fn is_unsupported_function_like(&self) -> bool {
        self.category == MacroCategory::Unsupported && self.form == MacroForm::FunctionLike
    }

    pub fn is_unsupported_object_like(&self) -> bool {
        self.category == MacroCategory::Unsupported && self.form == MacroForm::ObjectLike
    }
}

/// Per-macro provenance attached at the package layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MacroProvenance {
    pub macro_name: String,
    #[serde(default)]
    pub source_origin: Option<SourceOrigin>,
    #[serde(default)]
    pub source_location: Option<SourceLocation>,
}

/// Filtered macro environment entry intended for ABI/configuration auditing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroEnvironmentEntry {
    pub macro_name: String,
    pub category: MacroCategory,
    #[serde(default)]
    pub value: Option<MacroValue>,
    #[serde(default)]
    pub source_origin: Option<SourceOrigin>,
    #[serde(default)]
    pub source_location: Option<SourceLocation>,
}
