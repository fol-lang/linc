//! Link surface types for the LINC IR.
//!
//! These types represent native link requirements, artifact metadata, and
//! the normalized link surface attached to a binding package.

use serde::{Deserialize, Serialize};

/// Declared preference for how a library-name input should be linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkLibraryKind {
    Default,
    Static,
    Dynamic,
}

/// Package-level preference for static vs dynamic resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LinkResolutionMode {
    #[default]
    Default,
    PreferStatic,
    PreferDynamic,
}

/// Coarse classification of the native surface attached to a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NativeSurfaceKind {
    #[default]
    HeaderOnly,
    LibraryNames,
    ConcreteArtifacts,
    Mixed,
}

/// Provenance of one native requirement in the normalized link surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LinkRequirementSource {
    #[default]
    Declared,
    Inferred,
    Discovered,
}

/// One library-name link requirement.
///
/// Invariant: `name` is the linker-visible library identifier without platform search resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkLibrary {
    pub name: String,
    pub kind: LinkLibraryKind,
    #[serde(default)]
    pub source: LinkRequirementSource,
}

/// Concrete kind of a declared native artifact requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkArtifactKind {
    Object,
    StaticLibrary,
    SharedLibrary,
}

/// One concrete native artifact requirement.
///
/// Invariant: `path` is consumer-provided metadata and is not rewritten into a canonical resolved
/// filesystem path by this type alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkArtifact {
    pub path: String,
    pub kind: LinkArtifactKind,
    #[serde(default)]
    pub source: LinkRequirementSource,
}

/// One Apple framework requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkFramework {
    pub name: String,
    #[serde(default)]
    pub source: LinkRequirementSource,
}

/// Ordered native input as originally declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkInput {
    Library(LinkLibrary),
    Artifact(LinkArtifact),
    Framework(LinkFramework),
}

/// Normalized native link surface attached to a binding package.
///
/// Invariant: bucketed collections and `ordered_inputs` are both preserved because ordering and
/// categorization serve different downstream uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BindingLinkSurface {
    #[serde(default)]
    pub preferred_mode: LinkResolutionMode,
    #[serde(default)]
    pub native_surface_kind: NativeSurfaceKind,
    #[serde(default)]
    pub platform_constraints: Vec<String>,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub framework_paths: Vec<String>,
    #[serde(default)]
    pub library_paths: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<LinkLibrary>,
    #[serde(default)]
    pub frameworks: Vec<LinkFramework>,
    #[serde(default)]
    pub artifacts: Vec<LinkArtifact>,
    #[serde(default)]
    pub ordered_inputs: Vec<LinkInput>,
}
