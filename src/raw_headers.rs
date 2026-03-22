//! Transitional raw-header bootstrap layer.
//!
//! This module exists so the repo can still start from real headers while
//! `parc` finishes owning the frontend boundary. It is not the long-term
//! preferred intake path for downstream consumers.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::error::LincError;
use crate::ir::{
    AbiConfidence, BindingDefine, BindingInputs, BindingItem, BindingItemKind, BindingLinkSurface,
    BindingPackage, BindingTarget, DeclarationProvenance, LinkArtifact, LinkArtifactKind,
    LinkFramework, LinkInput, LinkLibrary, LinkLibraryKind, LinkRequirementSource,
    LinkResolutionMode, MacroBinding, MacroCategory, MacroEnvironmentEntry, MacroForm, MacroKind,
    MacroProvenance, MacroValue, NativeSurfaceKind,
};
use crate::line_markers::{FileOriginMap, OriginFilter};
use crate::probe::ProbeSubjectReport;

/// High-level scan configuration for turning headers into a `BindingPackage`.
///
/// This type is transitional bootstrap infrastructure. New downstream code
/// should prefer `SourcePackage -> LinkAnalysisPackage` and only use
/// `HeaderConfig` when raw-header ownership genuinely belongs inside LINC.
///
/// Invariant: builder methods append in declaration order, and validation is expected to run before
/// preprocessing, extraction, or probing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderConfig {
    /// Entry-point headers that define the intended bind surface.
    pub entry_headers: Vec<PathBuf>,
    /// Preprocessor/header search inputs used to make those headers parse correctly.
    pub include_dirs: Vec<PathBuf>,
    /// Framework search roots for Apple-style native dependency declarations.
    pub framework_dirs: Vec<PathBuf>,
    /// Native library search roots preserved as part of the package link surface.
    pub library_dirs: Vec<PathBuf>,
    /// Preprocessor defines that shape the extracted API and ABI surface.
    pub defines: Vec<(String, Option<String>)>,
    /// Declared native library-name requirements.
    pub link_libraries: Vec<LinkLibrary>,
    /// Declared framework requirements.
    pub link_frameworks: Vec<LinkFramework>,
    /// Declared concrete native artifact requirements.
    pub link_artifacts: Vec<LinkArtifact>,
    /// The original declared link-input order across libraries, frameworks, and artifacts.
    pub ordered_link_inputs: Vec<LinkInput>,
    /// Policy preference for static vs dynamic resolution.
    pub preferred_link_mode: LinkResolutionMode,
    /// Package-level target applicability hints.
    pub platform_constraints: Vec<String>,
    /// Requested ABI probe subjects to attach to the produced package.
    pub probe_types: Vec<String>,
    /// Compiler or driver used for preprocessing and ABI probing.
    pub compiler: Option<String>,
    /// C dialect / compiler-flavor assumptions for preprocessing and parsing.
    pub flavor: Option<Flavor>,
    /// Post-extraction source-origin filtering policy.
    #[serde(skip)]
    pub origin_filter: Option<OriginFilter>,
}

/// Preprocessing/parser flavor used for scan and probe operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flavor {
    GnuC11,
    ClangC11,
    StdC11,
}

/// Effective preprocessing invocation details for one scan.
///
/// Invariant: this is execution provenance for the returned result, not a complete replayable build
/// graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingReport {
    pub command: String,
    pub args: Vec<String>,
    pub preprocessed_source: String,
}

/// Successful output of a raw-header scan.
///
/// Invariant: `package` is the durable machine contract while `report` is the execution evidence
/// that produced it.
#[derive(Debug)]
pub struct RawHeaderResult {
    pub package: BindingPackage,
    pub report: PreprocessingReport,
}

/// Borrowed view over the preprocessing-related portion of `HeaderConfig`.
#[derive(Debug, Clone, Copy)]
pub struct PreprocessingConfigRef<'a> {
    pub include_dirs: &'a [PathBuf],
    pub defines: &'a [(String, Option<String>)],
    pub compiler: Option<&'a str>,
    pub flavor: Option<Flavor>,
}

/// Borrowed view over the entry-header surface of `HeaderConfig`.
#[derive(Debug, Clone, Copy)]
pub struct BindingSurfaceConfigRef<'a> {
    pub entry_headers: &'a [PathBuf],
}

/// Borrowed view over the native-link portion of `HeaderConfig`.
#[derive(Debug, Clone, Copy)]
pub struct LinkConfigRef<'a> {
    pub framework_dirs: &'a [PathBuf],
    pub library_dirs: &'a [PathBuf],
    pub link_libraries: &'a [LinkLibrary],
    pub link_frameworks: &'a [LinkFramework],
    pub link_artifacts: &'a [LinkArtifact],
    pub ordered_link_inputs: &'a [LinkInput],
    pub preferred_link_mode: LinkResolutionMode,
    pub platform_constraints: &'a [String],
}

/// Borrowed view over the probe request portion of `HeaderConfig`.
#[derive(Debug, Clone, Copy)]
pub struct ProbeConfigRef<'a> {
    pub probe_types: &'a [String],
}

/// Lightweight summary of the mixed bootstrap surface still carried by `HeaderConfig`.
///
/// This is intentionally small and count-oriented so callers can reason about
/// which domains are active without treating the whole builder as one flat bag
/// of options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderConfigSummary {
    pub entry_header_count: usize,
    pub include_dir_count: usize,
    pub define_count: usize,
    pub declared_link_input_count: usize,
    pub probe_type_count: usize,
    pub native_surface_kind: NativeSurfaceKind,
    pub preferred_link_mode: LinkResolutionMode,
    pub has_origin_filter: bool,
}

impl HeaderConfig {
    /// Create a new scan configuration.
    ///
    /// Conceptually, this type currently groups five subdomains:
    ///
    /// 1. preprocessing inputs
    /// 2. binding-surface inputs
    /// 3. native link declarations
    /// 4. ABI probe requests
    /// 5. origin-filtering policy
    ///
    /// The implementation is still a single builder type, but downstream users
    /// should reason about configuration through those subdomains rather than as
    /// one flat bag of options.
    ///
    /// Defaults and precedence rules:
    ///
    /// - `origin_filter` defaults to `Some(OriginFilter::default())`
    /// - `preferred_link_mode` defaults to `LinkResolutionMode::Default`
    /// - `flavor` defaults to `Flavor::GnuC11`
    /// - `compiler` defaults to `clang` when the effective flavor is `ClangC11`,
    ///   otherwise `gcc`
    /// - repeated builder calls append in declaration order rather than replacing
    ///   previous values
    /// - explicit single-item builders and bulk builders follow the same append-only
    ///   semantics
    /// - `no_origin_filter()` disables filtering entirely and therefore takes precedence
    ///   over the implicit default filter
    /// - explicit `compiler(...)` and `flavor(...)` values are reflected in the produced
    ///   package target metadata and probe/preprocess invocation
    ///
    /// For consumers that want to treat those subdomains explicitly without a full type split
    /// yet, this API also exposes borrowed views via:
    ///
    /// - [`HeaderConfig::preprocessing`]
    /// - [`HeaderConfig::binding_surface`]
    /// - [`HeaderConfig::linking`]
    /// - [`HeaderConfig::probing`]
    /// - [`HeaderConfig::filtering`]
    ///
    /// Naming policy:
    ///
    /// - `new()` remains the constructor
    /// - short historical builders such as `header(...)`, `include_dir(...)`, and `link_lib(...)`
    ///   remain supported
    /// - clearer aliases such as `entry_header(...)`, `add_include_dir(...)`, `link_library(...)`,
    ///   `define_flag(...)`, and `define_value(...)` are the preferred naming style for new code
    pub fn new() -> Self {
        Self {
            entry_headers: Vec::new(),
            include_dirs: Vec::new(),
            framework_dirs: Vec::new(),
            library_dirs: Vec::new(),
            defines: Vec::new(),
            link_libraries: Vec::new(),
            link_frameworks: Vec::new(),
            link_artifacts: Vec::new(),
            ordered_link_inputs: Vec::new(),
            preferred_link_mode: LinkResolutionMode::Default,
            platform_constraints: Vec::new(),
            probe_types: Vec::new(),
            compiler: None,
            flavor: None,
            origin_filter: Some(OriginFilter::default()),
        }
    }

    pub fn header(mut self, path: impl Into<PathBuf>) -> Self {
        self.entry_headers.push(path.into());
        self
    }

    pub fn entry_header(self, path: impl Into<PathBuf>) -> Self {
        self.header(path)
    }

    pub fn headers<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.entry_headers.extend(paths.into_iter().map(Into::into));
        self
    }

    pub fn include_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.include_dirs.push(path.into());
        self
    }

    pub fn add_include_dir(self, path: impl Into<PathBuf>) -> Self {
        self.include_dir(path)
    }

    pub fn include_dirs<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.include_dirs.extend(paths.into_iter().map(Into::into));
        self
    }

    pub fn framework_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.framework_dirs.push(path.into());
        self
    }

    pub fn add_framework_dir(self, path: impl Into<PathBuf>) -> Self {
        self.framework_dir(path)
    }

    pub fn framework_dirs<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.framework_dirs
            .extend(paths.into_iter().map(Into::into));
        self
    }

    pub fn library_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.library_dirs.push(path.into());
        self
    }

    pub fn add_library_dir(self, path: impl Into<PathBuf>) -> Self {
        self.library_dir(path)
    }

    pub fn library_dirs<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.library_dirs.extend(paths.into_iter().map(Into::into));
        self
    }

    pub fn define(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        self.defines.push((name.into(), value));
        self
    }

    pub fn define_value(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.define(name, Some(value.into()))
    }

    pub fn define_flag(self, name: impl Into<String>) -> Self {
        self.define(name, None)
    }

    pub fn defines<I, N>(mut self, defines: I) -> Self
    where
        I: IntoIterator<Item = (N, Option<String>)>,
        N: Into<String>,
    {
        self.defines.extend(
            defines
                .into_iter()
                .map(|(name, value)| (name.into(), value)),
        );
        self
    }

    pub fn link_lib(mut self, name: impl Into<String>) -> Self {
        let library = LinkLibrary {
            name: name.into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Library(library.clone()));
        self.link_libraries.push(library);
        self
    }

    pub fn link_library(self, name: impl Into<String>) -> Self {
        self.link_lib(name)
    }

    pub fn link_static_lib(mut self, name: impl Into<String>) -> Self {
        let library = LinkLibrary {
            name: name.into(),
            kind: LinkLibraryKind::Static,
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Library(library.clone()));
        self.link_libraries.push(library);
        self
    }

    pub fn link_shared_lib(mut self, name: impl Into<String>) -> Self {
        let library = LinkLibrary {
            name: name.into(),
            kind: LinkLibraryKind::Dynamic,
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Library(library.clone()));
        self.link_libraries.push(library);
        self
    }

    pub fn link_object_file(mut self, path: impl Into<PathBuf>) -> Self {
        let artifact = LinkArtifact {
            path: path.into().display().to_string(),
            kind: LinkArtifactKind::Object,
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Artifact(artifact.clone()));
        self.link_artifacts.push(artifact);
        self
    }

    pub fn link_framework(mut self, name: impl Into<String>) -> Self {
        let framework = LinkFramework {
            name: name.into(),
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Framework(framework.clone()));
        self.link_frameworks.push(framework);
        self
    }

    pub fn request_probe_type_layout(self, name: impl Into<String>) -> Self {
        self.probe_type_layout(name)
    }

    pub fn link_static_artifact(mut self, path: impl Into<PathBuf>) -> Self {
        let artifact = LinkArtifact {
            path: path.into().display().to_string(),
            kind: LinkArtifactKind::StaticLibrary,
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Artifact(artifact.clone()));
        self.link_artifacts.push(artifact);
        self
    }

    pub fn link_shared_artifact(mut self, path: impl Into<PathBuf>) -> Self {
        let artifact = LinkArtifact {
            path: path.into().display().to_string(),
            kind: LinkArtifactKind::SharedLibrary,
            source: LinkRequirementSource::Declared,
        };
        self.ordered_link_inputs
            .push(LinkInput::Artifact(artifact.clone()));
        self.link_artifacts.push(artifact);
        self
    }

    pub fn prefer_static_linking(mut self) -> Self {
        self.preferred_link_mode = LinkResolutionMode::PreferStatic;
        self
    }

    pub fn prefer_dynamic_linking(mut self) -> Self {
        self.preferred_link_mode = LinkResolutionMode::PreferDynamic;
        self
    }

    pub fn target_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.platform_constraints.push(constraint.into());
        self
    }

    pub fn target_constraints<I, S>(mut self, constraints: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.platform_constraints
            .extend(constraints.into_iter().map(Into::into));
        self
    }

    pub fn probe_type_layout(mut self, name: impl Into<String>) -> Self {
        self.probe_types.push(name.into());
        self
    }

    pub fn probe_type_layouts<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.probe_types.extend(names.into_iter().map(Into::into));
        self
    }

    pub fn compiler(mut self, cmd: impl Into<String>) -> Self {
        self.compiler = Some(cmd.into());
        self
    }

    pub fn flavor(mut self, flavor: Flavor) -> Self {
        self.flavor = Some(flavor);
        self
    }

    pub fn origin_filter(mut self, filter: OriginFilter) -> Self {
        self.origin_filter = Some(filter);
        self
    }

    pub fn no_origin_filter(mut self) -> Self {
        self.origin_filter = None;
        self
    }

    pub fn preprocessing(&self) -> PreprocessingConfigRef<'_> {
        PreprocessingConfigRef {
            include_dirs: &self.include_dirs,
            defines: &self.defines,
            compiler: self.compiler.as_deref(),
            flavor: self.flavor,
        }
    }

    pub fn binding_surface(&self) -> BindingSurfaceConfigRef<'_> {
        BindingSurfaceConfigRef {
            entry_headers: &self.entry_headers,
        }
    }

    pub fn linking(&self) -> LinkConfigRef<'_> {
        LinkConfigRef {
            framework_dirs: &self.framework_dirs,
            library_dirs: &self.library_dirs,
            link_libraries: &self.link_libraries,
            link_frameworks: &self.link_frameworks,
            link_artifacts: &self.link_artifacts,
            ordered_link_inputs: &self.ordered_link_inputs,
            preferred_link_mode: self.preferred_link_mode,
            platform_constraints: &self.platform_constraints,
        }
    }

    pub fn probing(&self) -> ProbeConfigRef<'_> {
        ProbeConfigRef {
            probe_types: &self.probe_types,
        }
    }

    pub fn filtering(&self) -> Option<&OriginFilter> {
        self.origin_filter.as_ref()
    }

    pub fn summary(&self) -> HeaderConfigSummary {
        HeaderConfigSummary {
            entry_header_count: self.entry_headers.len(),
            include_dir_count: self.include_dirs.len(),
            define_count: self.defines.len(),
            declared_link_input_count: self.ordered_link_inputs.len(),
            probe_type_count: self.probe_types.len(),
            native_surface_kind: self.native_surface_kind(),
            preferred_link_mode: self.preferred_link_mode,
            has_origin_filter: self.origin_filter.is_some(),
        }
    }

    pub fn validate(&self) -> Result<(), LincError> {
        if self.entry_headers.is_empty() {
            return Err(LincError::NoHeaders);
        }

        fn invalid(reason: impl Into<String>) -> LincError {
            LincError::InvalidConfig {
                reason: reason.into(),
            }
        }

        if self
            .entry_headers
            .iter()
            .any(|path| path.as_os_str().is_empty())
        {
            return Err(invalid("entry header path must not be empty"));
        }
        if self
            .include_dirs
            .iter()
            .any(|path| path.as_os_str().is_empty())
        {
            return Err(invalid("include directory path must not be empty"));
        }
        if self
            .framework_dirs
            .iter()
            .any(|path| path.as_os_str().is_empty())
        {
            return Err(invalid("framework directory path must not be empty"));
        }
        if self
            .library_dirs
            .iter()
            .any(|path| path.as_os_str().is_empty())
        {
            return Err(invalid("library directory path must not be empty"));
        }
        if self.compiler.as_deref().is_some_and(str::is_empty) {
            return Err(invalid("compiler command must not be empty"));
        }
        if self.defines.iter().any(|(name, _)| name.is_empty()) {
            return Err(invalid("define name must not be empty"));
        }
        if self.probe_types.iter().any(String::is_empty) {
            return Err(invalid("probe type name must not be empty"));
        }
        if self.platform_constraints.iter().any(String::is_empty) {
            return Err(invalid("target constraint must not be empty"));
        }
        if self
            .link_libraries
            .iter()
            .any(|library| library.name.is_empty())
        {
            return Err(invalid("link library name must not be empty"));
        }
        if self
            .link_frameworks
            .iter()
            .any(|framework| framework.name.is_empty())
        {
            return Err(invalid("link framework name must not be empty"));
        }
        if self
            .link_artifacts
            .iter()
            .any(|artifact| artifact.path.is_empty())
        {
            return Err(invalid("link artifact path must not be empty"));
        }

        Ok(())
    }

    pub fn binding_target(&self) -> BindingTarget {
        let compiler_command = self.compiler_command();
        BindingTarget {
            target_triple: detect_target_triple(&compiler_command),
            compiler_command: Some(compiler_command.clone()),
            compiler_version: detect_compiler_version(&compiler_command),
            flavor: Some(self.flavor_label()),
        }
    }

    pub fn binding_inputs(&self) -> BindingInputs {
        BindingInputs {
            entry_headers: self
                .entry_headers
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            include_dirs: self
                .include_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            defines: self
                .defines
                .iter()
                .map(|(name, value)| BindingDefine {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
        }
    }

    pub fn binding_link_surface(&self) -> BindingLinkSurface {
        BindingLinkSurface {
            preferred_mode: self.preferred_link_mode,
            native_surface_kind: self.native_surface_kind(),
            platform_constraints: self.platform_constraints.clone(),
            include_paths: self
                .include_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            framework_paths: self
                .framework_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            library_paths: self
                .library_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            libraries: self.link_libraries.clone(),
            frameworks: self.link_frameworks.clone(),
            artifacts: self.link_artifacts.clone(),
            ordered_inputs: self.ordered_link_inputs.clone(),
        }
    }

    pub fn native_surface_kind(&self) -> NativeSurfaceKind {
        match (
            self.link_libraries.is_empty() && self.link_frameworks.is_empty(),
            self.link_artifacts.is_empty(),
        ) {
            (true, true) => NativeSurfaceKind::HeaderOnly,
            (false, true) => NativeSurfaceKind::LibraryNames,
            (true, false) => NativeSurfaceKind::ConcreteArtifacts,
            (false, false) => NativeSurfaceKind::Mixed,
        }
    }

    pub fn compiler_command(&self) -> String {
        let flavor = self.flavor.unwrap_or(Flavor::GnuC11);
        self.compiler.clone().unwrap_or_else(|| match flavor {
            Flavor::ClangC11 => "clang".into(),
            _ => "gcc".into(),
        })
    }

    pub fn flavor_label(&self) -> String {
        match self.flavor.unwrap_or(Flavor::GnuC11) {
            Flavor::GnuC11 => "gnu-c11".into(),
            Flavor::ClangC11 => "clang-c11".into(),
            Flavor::StdC11 => "std-c11".into(),
        }
    }

    pub fn build_combined_source(&self) -> String {
        let mut source = String::new();
        for header in &self.entry_headers {
            source.push_str(&format!("#include \"{}\"\n", header.display()));
        }
        source
    }

    pub fn attach_requested_probes(&self, package: &mut BindingPackage) -> Result<(), LincError> {
        use crate::probe::{probe_type_layouts_with_fields, ProbedFieldSpec};
        if self.probe_types.is_empty() {
            return Ok(());
        }

        let mut field_specs = std::collections::BTreeMap::new();
        for item in &package.items {
            if let BindingItem::Record(record) = item {
                let Some(name) = &record.name else { continue };
                let prefix = match record.kind {
                    crate::ir::RecordKind::Struct => "struct",
                    crate::ir::RecordKind::Union => "union",
                };
                let Some(fields) = &record.fields else {
                    continue;
                };
                let specs: Vec<ProbedFieldSpec> = fields
                    .iter()
                    .filter_map(|f| {
                        Some(ProbedFieldSpec {
                            name: f.name.as_ref()?.clone(),
                            bit_width: f.bit_width,
                        })
                    })
                    .collect();
                if !specs.is_empty() {
                    field_specs.insert(format!("{} {}", prefix, name), specs);
                }
            }
        }

        let probe_config = crate::probe::ProbeConfig::from(self);
        match probe_type_layouts_with_fields(&probe_config, &self.probe_types, &field_specs) {
            Ok(probe_report) => {
                package.layouts = probe_report.layouts;
                attach_probe_evidence(&mut package.items, &probe_report.subjects);
                Ok(())
            }
            Err(error @ LincError::InvalidConfig { .. }) => Err(error),
            Err(error @ LincError::NoProbeTypes) => Err(error),
            Err(error) => {
                package.diagnostics.push(Diagnostic::warning(
                    classify_probe_diagnostic_kind(&error),
                    format!("probe failed: {}", error),
                ));
                Ok(())
            }
        }
    }

    pub fn capture_macros(&self, input: &Path) -> (Vec<MacroBinding>, Vec<MacroProvenance>) {
        let compiler = self.compiler_command();
        let mut cmd = std::process::Command::new(&compiler);
        cmd.arg("-dD").arg("-E");
        for dir in &self.include_dirs {
            cmd.arg(format!("-I{}", dir.display()));
        }
        for (name, value) in &self.defines {
            match value {
                Some(v) => {
                    cmd.arg(format!("-D{}={}", name, v));
                }
                None => {
                    cmd.arg(format!("-D{}", name));
                }
            }
        }
        cmd.arg(input);

        let Ok(output) = cmd.output() else {
            return (Vec::new(), Vec::new());
        };
        if !output.status.success() {
            return (Vec::new(), Vec::new());
        }
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return (Vec::new(), Vec::new());
        };
        parse_macro_definitions_with_provenance(&stdout, &self.entry_headers)
    }
}

pub fn classify_probe_diagnostic_kind(error: &LincError) -> DiagnosticKind {
    match error {
        LincError::ProbeCompile { stderr, .. }
            if stderr.contains("incomplete type")
                || stderr.contains("incomplete typedef")
                || stderr.contains("invalid application of ‘sizeof’")
                || stderr.contains("invalid application of ‘alignof’")
                || stderr.contains("invalid application of ‘sizeof’")
                || stderr.contains("invalid application of ‘_Alignof’")
                || stderr.contains("invalid application of ‘__alignof__’") =>
        {
            DiagnosticKind::ProbeUnavailable
        }
        _ => DiagnosticKind::ProbeFailed,
    }
}

pub struct RecoveredParse {
    pub source: String,
    pub items: Vec<BindingItem>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn sanitize_attribute_bearing_record_typedefs(source: &str) -> Option<String> {
    let patterns = [
        ("typedef struct __attribute__((packed)) ", "typedef struct "),
        (
            "typedef struct __attribute__((__packed__)) ",
            "typedef struct ",
        ),
        (
            "typedef struct __attribute__((aligned(16))) ",
            "typedef struct ",
        ),
        (
            "typedef struct __attribute__((__aligned__(16))) ",
            "typedef struct ",
        ),
        ("typedef union __attribute__((packed)) ", "typedef union "),
        (
            "typedef union __attribute__((__packed__)) ",
            "typedef union ",
        ),
        (
            "typedef union __attribute__((aligned(16))) ",
            "typedef union ",
        ),
        (
            "typedef union __attribute__((__aligned__(16))) ",
            "typedef union ",
        ),
    ];

    let mut sanitized = source.to_string();
    let mut changed = false;
    for (pattern, replacement) in patterns {
        if sanitized.contains(pattern) {
            sanitized = sanitized.replace(pattern, replacement);
            changed = true;
        }
    }

    changed.then_some(sanitized)
}

pub fn attach_canonical_alias_resolution(items: &mut [BindingItem]) {
    let alias_map = items
        .iter()
        .filter_map(|item| match item {
            BindingItem::TypeAlias(alias) => Some((alias.name.clone(), alias.target.clone())),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();

    for item in items {
        let BindingItem::TypeAlias(alias) = item else {
            continue;
        };
        alias.canonical_resolution = resolve_alias_resolution(&alias.target, &alias_map);
    }
}

pub fn resolve_alias_resolution(
    ty: &crate::ir::BindingType,
    alias_map: &std::collections::HashMap<String, crate::ir::BindingType>,
) -> Option<crate::ir::AliasResolution> {
    let mut alias_chain = Vec::new();
    let terminal_target = canonicalize_binding_type(
        ty,
        alias_map,
        &mut alias_chain,
        &mut std::collections::HashSet::new(),
    )?;
    if alias_chain.is_empty() {
        None
    } else {
        Some(crate::ir::AliasResolution {
            alias_chain,
            terminal_target,
        })
    }
}

pub fn canonicalize_binding_type(
    ty: &crate::ir::BindingType,
    alias_map: &std::collections::HashMap<String, crate::ir::BindingType>,
    alias_chain: &mut Vec<String>,
    seen_aliases: &mut std::collections::HashSet<String>,
) -> Option<crate::ir::BindingType> {
    match ty {
        crate::ir::BindingType::TypedefRef(name) => {
            let resolved = alias_map.get(name)?;
            if !seen_aliases.insert(name.clone()) {
                return None;
            }
            alias_chain.push(name.clone());
            canonicalize_binding_type(resolved, alias_map, alias_chain, seen_aliases)
                .or_else(|| Some(resolved.clone()))
        }
        crate::ir::BindingType::Pointer {
            pointee,
            const_pointee,
            qualifiers,
        } => canonicalize_binding_type(pointee, alias_map, alias_chain, seen_aliases).map(
            |resolved| crate::ir::BindingType::Pointer {
                pointee: Box::new(resolved),
                const_pointee: *const_pointee,
                qualifiers: *qualifiers,
            },
        ),
        crate::ir::BindingType::Qualified { ty, qualifiers } => {
            canonicalize_binding_type(ty, alias_map, alias_chain, seen_aliases).map(|resolved| {
                crate::ir::BindingType::Qualified {
                    ty: Box::new(resolved),
                    qualifiers: *qualifiers,
                }
            })
        }
        crate::ir::BindingType::Array(inner, len) => {
            canonicalize_binding_type(inner, alias_map, alias_chain, seen_aliases)
                .map(|resolved| crate::ir::BindingType::Array(Box::new(resolved), *len))
        }
        crate::ir::BindingType::FunctionPointer {
            return_type,
            parameters,
            variadic,
        } => {
            let resolved_return =
                canonicalize_binding_type(return_type, alias_map, alias_chain, seen_aliases)
                    .unwrap_or((**return_type).clone());
            let resolved_parameters = parameters
                .iter()
                .map(|parameter| {
                    canonicalize_binding_type(parameter, alias_map, alias_chain, seen_aliases)
                        .unwrap_or_else(|| parameter.clone())
                })
                .collect();
            Some(crate::ir::BindingType::FunctionPointer {
                return_type: Box::new(resolved_return),
                parameters: resolved_parameters,
                variadic: *variadic,
            })
        }
        other => Some(other.clone()),
    }
}

pub fn build_item_provenance(
    items: &[BindingItem],
    origin_map: &FileOriginMap,
) -> Vec<DeclarationProvenance> {
    items
        .iter()
        .map(|item| {
            let (item_name, item_kind, source_offset) = match item {
                BindingItem::Function(f) => (
                    Some(f.name.clone()),
                    BindingItemKind::Function,
                    f.source_offset,
                ),
                BindingItem::Record(r) => {
                    (r.name.clone(), BindingItemKind::Record, r.source_offset)
                }
                BindingItem::Enum(e) => (e.name.clone(), BindingItemKind::Enum, e.source_offset),
                BindingItem::TypeAlias(t) => (
                    Some(t.name.clone()),
                    BindingItemKind::TypeAlias,
                    t.source_offset,
                ),
                BindingItem::Variable(v) => (
                    Some(v.name.clone()),
                    BindingItemKind::Variable,
                    v.source_offset,
                ),
                BindingItem::Unsupported(u) => (
                    u.name.clone(),
                    BindingItemKind::Unsupported,
                    u.source_offset,
                ),
            };

            DeclarationProvenance {
                item_name,
                item_kind: Some(item_kind),
                source_offset,
                source_origin: source_offset.map(|offset| origin_map.origin_at(offset)),
                source_location: source_offset.and_then(|offset| origin_map.location_at(offset)),
            }
        })
        .collect()
}

pub fn build_effective_macro_environment(
    macros: &[MacroBinding],
    provenance: &[MacroProvenance],
) -> Vec<MacroEnvironmentEntry> {
    macros
        .iter()
        .enumerate()
        .filter(|(_, binding)| {
            matches!(
                binding.category,
                MacroCategory::ConfigurationFlag | MacroCategory::AbiAffecting
            )
        })
        .map(|(index, binding)| {
            let provenance = provenance.get(index);
            MacroEnvironmentEntry {
                macro_name: binding.name.clone(),
                category: binding.category.clone(),
                value: binding.value.clone(),
                source_origin: provenance.and_then(|prov| prov.source_origin.clone()),
                source_location: provenance.and_then(|prov| prov.source_location.clone()),
            }
        })
        .collect()
}

pub fn attach_probe_evidence(items: &mut [BindingItem], subjects: &[ProbeSubjectReport]) {
    let subject_map = subjects
        .iter()
        .map(|subject| (subject.name.as_str(), subject))
        .collect::<std::collections::HashMap<_, _>>();

    for item in items {
        match item {
            BindingItem::TypeAlias(type_alias) => {
                if let Some(subject) = subject_map.get(type_alias.name.as_str()) {
                    type_alias.abi_confidence = Some(alias_abi_confidence(subject));
                }
            }
            BindingItem::Record(record) => {
                let Some(record_name) = record.name.as_deref() else {
                    continue;
                };
                let subject_name = match record.kind {
                    crate::ir::RecordKind::Struct => format!("struct {}", record_name),
                    crate::ir::RecordKind::Union => format!("union {}", record_name),
                };
                let Some(subject) = subject_map.get(subject_name.as_str()) else {
                    continue;
                };
                record.representation = Some(crate::ir::RecordRepresentation {
                    size: Some(subject.layout.size),
                    align: Some(subject.layout.align),
                    completeness: subject
                        .record_completeness
                        .map(|completeness| format!("{:?}", completeness)),
                });
                record.abi_confidence = Some(record_abi_confidence(subject));
                let Some(fields) = record.fields.as_mut() else {
                    continue;
                };
                let field_map = subject
                    .fields
                    .iter()
                    .map(|field| (field.name.as_str(), field))
                    .collect::<std::collections::HashMap<_, _>>();
                for field in fields {
                    let Some(field_name) = field.name.as_deref() else {
                        continue;
                    };
                    let Some(probed) = field_map.get(field_name) else {
                        continue;
                    };
                    field.layout = Some(crate::ir::FieldLayout {
                        offset_bytes: probed.offset_bytes,
                    });
                }
            }
            BindingItem::Enum(enum_binding) => {
                let Some(enum_name) = enum_binding.name.as_deref() else {
                    continue;
                };
                let subject_name = format!("enum {}", enum_name);
                let Some(subject) = subject_map.get(subject_name.as_str()) else {
                    continue;
                };
                enum_binding.representation = Some(crate::ir::EnumRepresentation {
                    underlying_size: subject.enum_underlying_size,
                    is_signed: subject.enum_is_signed,
                });
                enum_binding.abi_confidence = Some(enum_abi_confidence(subject));
            }
            _ => {}
        }
    }
}

pub fn alias_abi_confidence(subject: &ProbeSubjectReport) -> AbiConfidence {
    let _ = subject;
    AbiConfidence::LayoutProbed
}

pub fn enum_abi_confidence(subject: &ProbeSubjectReport) -> AbiConfidence {
    if subject.enum_underlying_size.is_some() || subject.enum_is_signed.is_some() {
        AbiConfidence::RepresentationProbed
    } else {
        AbiConfidence::LayoutProbed
    }
}

pub fn record_abi_confidence(subject: &ProbeSubjectReport) -> AbiConfidence {
    if subject
        .fields
        .iter()
        .any(|field| field.bit_width.is_some() && field.offset_bytes.is_none())
    {
        AbiConfidence::PartialBitfieldLayout
    } else if subject
        .fields
        .iter()
        .any(|field| field.offset_bytes.is_some())
    {
        AbiConfidence::FieldOffsetsProbed
    } else if subject.record_completeness.is_some() {
        AbiConfidence::RepresentationProbed
    } else {
        AbiConfidence::LayoutProbed
    }
}

pub(crate) fn detect_target_triple(compiler_command: &str) -> Option<String> {
    let output = std::process::Command::new(compiler_command)
        .arg("-dumpmachine")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let triple = String::from_utf8(output.stdout).ok()?;
    let triple = triple.trim();
    if triple.is_empty() {
        None
    } else {
        Some(triple.to_string())
    }
}

pub(crate) fn detect_compiler_version(compiler_command: &str) -> Option<String> {
    let output = std::process::Command::new(compiler_command)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .next()
        .map(str::to_string)
        .filter(|line| !line.is_empty())
}

#[cfg(test)]
fn parse_macro_definitions(source: &str) -> Vec<MacroBinding> {
    source
        .lines()
        .filter_map(parse_macro_definition_line)
        .collect()
}

pub fn parse_macro_definitions_with_provenance(
    source: &str,
    entry_headers: &[impl AsRef<Path>],
) -> (Vec<MacroBinding>, Vec<MacroProvenance>) {
    let origin_map = FileOriginMap::parse(source, entry_headers);
    let mut macros = Vec::new();
    let mut provenance = Vec::new();
    let mut offset = 0;

    for line in source.split('\n') {
        let line_start = offset;
        offset += line.len() + 1;
        if let Some(binding) = parse_macro_definition_line(line) {
            provenance.push(MacroProvenance {
                macro_name: binding.name.clone(),
                source_origin: Some(origin_map.origin_at(line_start)),
                source_location: origin_map.location_at(line_start),
            });
            macros.push(binding);
        }
    }

    (macros, provenance)
}

pub fn parse_macro_definition_line(line: &str) -> Option<MacroBinding> {
    let line = line.trim();
    let rest = line.strip_prefix("#define ")?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let head = parts.next()?.trim();
    let body = parts.next().unwrap_or("").trim().to_string();

    let function_like = head.contains('(');
    let name = if function_like {
        head.split('(').next()?.trim().to_string()
    } else {
        head.to_string()
    };

    if name.is_empty() {
        return None;
    }

    Some(MacroBinding {
        kind: classify_macro_body(&body, function_like),
        category: classify_macro_category(&name, &body, function_like),
        value: parse_macro_value(&body, function_like),
        name,
        body,
        function_like,
        form: if function_like {
            MacroForm::FunctionLike
        } else {
            MacroForm::ObjectLike
        },
    })
}

pub fn parse_macro_value(body: &str, function_like: bool) -> Option<MacroValue> {
    if function_like {
        return None;
    }

    match classify_macro_body(body, function_like) {
        MacroKind::Integer => parse_integer_macro_value(body).map(MacroValue::Integer),
        MacroKind::String => parse_string_macro_value(body).map(MacroValue::String),
        _ => None,
    }
}

pub fn parse_integer_macro_value(body: &str) -> Option<i128> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (negative, unsigned) = match trimmed.as_bytes()[0] {
        b'+' => (false, &trimmed[1..]),
        b'-' => (true, &trimmed[1..]),
        _ => (false, trimmed),
    };

    let digits = unsigned.trim_end_matches(|ch: char| matches!(ch, 'u' | 'U' | 'l' | 'L'));
    if digits.is_empty() {
        return None;
    }

    let parsed = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i128::from_str_radix(rest, 16).ok()?
    } else if digits.len() > 1 && digits.starts_with('0') {
        i128::from_str_radix(&digits[1..], 8).ok()?
    } else {
        digits.parse::<i128>().ok()?
    };

    if negative {
        Some(-parsed)
    } else {
        Some(parsed)
    }
}

pub fn parse_string_macro_value(body: &str) -> Option<String> {
    let trimmed = body.trim();
    let inner = trimmed.strip_prefix('"')?.strip_suffix('"')?;
    Some(
        inner
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace("\\n", "\n")
            .replace("\\t", "\t"),
    )
}

pub fn classify_macro_body(body: &str, function_like: bool) -> MacroKind {
    if function_like {
        return MacroKind::Other;
    }

    if body.starts_with('"') && body.ends_with('"') && body.len() >= 2 {
        return MacroKind::String;
    }

    let trimmed = body.trim();
    if !trimmed.is_empty()
        && trimmed.chars().all(|ch| {
            ch.is_ascii_hexdigit() || matches!(ch, 'x' | 'X' | 'u' | 'U' | 'l' | 'L' | '+' | '-')
        })
    {
        return MacroKind::Integer;
    }

    if trimmed.chars().any(|ch| "+-*/%<>&|^!()".contains(ch)) {
        return MacroKind::Expression;
    }

    MacroKind::Other
}

pub fn classify_macro_category(name: &str, body: &str, function_like: bool) -> MacroCategory {
    if function_like {
        if name.contains("CALL") || name.contains("EXPORT") || name.contains("IMPORT") {
            return MacroCategory::AbiAffecting;
        }
        return MacroCategory::Unsupported;
    }

    if name.starts_with("HAVE_")
        || name.ends_with("_ENABLED")
        || name.ends_with("_DISABLED")
        || body == "0"
        || body == "1"
    {
        return MacroCategory::ConfigurationFlag;
    }

    let body_lower = body.to_ascii_lowercase();

    if name.contains("CALL")
        || name.contains("EXPORT")
        || name.contains("IMPORT")
        || name.contains("ALIGN")
        || name.contains("PACK")
        || name.contains("INLINE")
        || name.contains("WEAK")
        || name.contains("VISIBILITY")
        || body.contains("__attribute__")
        || body.contains("__declspec")
        || body.contains("__stdcall")
        || body.contains("__cdecl")
        || body_lower.contains("visibility")
    {
        return MacroCategory::AbiAffecting;
    }

    match classify_macro_body(body, function_like) {
        MacroKind::Integer | MacroKind::String | MacroKind::Expression => {
            MacroCategory::BindableConstant
        }
        MacroKind::Other => MacroCategory::Unsupported,
    }
}

impl Default for HeaderConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    #[test]
    fn config_builder() {
        let cfg = HeaderConfig::new()
            .header("foo.h")
            .include_dir("/usr/include")
            .framework_dir("/System/Library/Frameworks")
            .library_dir("/usr/lib")
            .define("DEBUG", None)
            .define("VERSION", Some("2".into()))
            .link_lib("m")
            .link_framework("Security")
            .link_static_lib("z")
            .link_shared_lib("ssl")
            .link_object_file("build/plugin.o")
            .link_static_artifact("native/libfoo.a")
            .link_shared_artifact("native/libfoo.so")
            .prefer_static_linking()
            .target_constraint("linux")
            .probe_type_layout("struct foo")
            .compiler("gcc")
            .flavor(Flavor::GnuC11);

        assert_eq!(cfg.entry_headers.len(), 1);
        assert_eq!(cfg.include_dirs.len(), 1);
        assert_eq!(cfg.framework_dirs.len(), 1);
        assert_eq!(cfg.library_dirs.len(), 1);
        assert_eq!(cfg.defines.len(), 2);
        assert_eq!(cfg.link_libraries.len(), 3);
        assert_eq!(cfg.link_frameworks.len(), 1);
        assert_eq!(cfg.link_artifacts.len(), 3);
        assert_eq!(cfg.preferred_link_mode, LinkResolutionMode::PreferStatic);
        assert_eq!(cfg.platform_constraints, vec!["linux".to_string()]);
        assert_eq!(cfg.native_surface_kind(), NativeSurfaceKind::Mixed);
        assert_eq!(cfg.probe_types.len(), 1);
        assert_eq!(cfg.compiler.as_deref(), Some("gcc"));
        assert_eq!(cfg.flavor, Some(Flavor::GnuC11));
    }

    #[test]
    fn no_headers_error() {
        let cfg = HeaderConfig::new();
        let result = cfg.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LincError::NoHeaders));
    }

    #[test]
    fn config_serialization() {
        let cfg = HeaderConfig::new()
            .header("test.h")
            .include_dir("/usr/local/include")
            .framework_dir("/System/Library/Frameworks")
            .library_dir("/usr/local/lib")
            .define("FOO", Some("1".into()))
            .link_framework("Foundation")
            .link_shared_lib("ssl")
            .link_shared_artifact("/usr/local/lib/libssl.so")
            .prefer_dynamic_linking()
            .target_constraint("macos")
            .probe_type_layout("size_t");

        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: HeaderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg2.entry_headers.len(), 1);
        assert_eq!(cfg2.defines.len(), 1);
        assert_eq!(cfg2.framework_dirs.len(), 1);
        assert_eq!(cfg2.library_dirs.len(), 1);
        assert_eq!(cfg2.link_frameworks.len(), 1);
        assert_eq!(cfg2.link_libraries.len(), 1);
        assert_eq!(cfg2.link_artifacts.len(), 1);
        assert_eq!(cfg2.ordered_link_inputs.len(), 3);
        assert_eq!(cfg2.preferred_link_mode, LinkResolutionMode::PreferDynamic);
        assert_eq!(cfg2.platform_constraints, vec!["macos".to_string()]);
        assert_eq!(cfg2.probe_types.len(), 1);
    }

    #[test]
    fn bulk_config_builders_append_in_declared_order() {
        let cfg = HeaderConfig::new()
            .headers(["a.h", "b.h"])
            .include_dirs(["/usr/include", "/usr/local/include"])
            .framework_dirs(["/System/Library/Frameworks", "/Library/Frameworks"])
            .library_dirs(["/usr/lib", "/usr/local/lib"])
            .defines([("DEBUG", None), ("VERSION", Some("2".to_string()))])
            .target_constraints(["linux", "x86_64"])
            .probe_type_layouts(["size_t", "struct stat"]);

        assert_eq!(
            cfg.entry_headers,
            vec![PathBuf::from("a.h"), PathBuf::from("b.h")]
        );
        assert_eq!(
            cfg.include_dirs,
            vec![
                PathBuf::from("/usr/include"),
                PathBuf::from("/usr/local/include")
            ]
        );
        assert_eq!(
            cfg.framework_dirs,
            vec![
                PathBuf::from("/System/Library/Frameworks"),
                PathBuf::from("/Library/Frameworks")
            ]
        );
        assert_eq!(
            cfg.library_dirs,
            vec![PathBuf::from("/usr/lib"), PathBuf::from("/usr/local/lib")]
        );
        assert_eq!(
            cfg.defines,
            vec![
                ("DEBUG".to_string(), None),
                ("VERSION".to_string(), Some("2".to_string()))
            ]
        );
        assert_eq!(
            cfg.platform_constraints,
            vec!["linux".to_string(), "x86_64".to_string()]
        );
        assert_eq!(
            cfg.probe_types,
            vec!["size_t".to_string(), "struct stat".to_string()]
        );
    }

    #[test]
    fn config_normalization_is_deterministic() {
        let cfg = HeaderConfig::new()
            .header("api.h")
            .include_dir("/usr/include")
            .framework_dir("/System/Library/Frameworks")
            .library_dir("/usr/lib")
            .define("DEBUG", None)
            .define("VERSION", Some("2".into()))
            .link_framework("Security")
            .link_static_lib("z")
            .link_shared_artifact("/usr/lib/libssl.so")
            .prefer_dynamic_linking()
            .target_constraint("linux")
            .probe_type_layout("size_t")
            .compiler("clang")
            .flavor(Flavor::ClangC11);

        let target_a = cfg.binding_target();
        let target_b = cfg.binding_target();
        assert_eq!(target_a, target_b);

        let inputs_a = cfg.binding_inputs();
        let inputs_b = cfg.binding_inputs();
        assert_eq!(inputs_a, inputs_b);

        let link_a = cfg.binding_link_surface();
        let link_b = cfg.binding_link_surface();
        assert_eq!(link_a, link_b);
    }

    #[test]
    fn config_domain_views_reflect_shared_config() {
        let cfg = HeaderConfig::new()
            .entry_header("api.h")
            .add_include_dir("/usr/include")
            .add_framework_dir("/System/Library/Frameworks")
            .add_library_dir("/usr/lib")
            .define_flag("DEBUG")
            .define_value("VERSION", "2")
            .link_library("z")
            .link_framework("Security")
            .link_shared_artifact("/usr/lib/libssl.so")
            .target_constraint("linux")
            .request_probe_type_layout("size_t");

        let preprocessing = cfg.preprocessing();
        assert_eq!(preprocessing.include_dirs, &[PathBuf::from("/usr/include")]);
        assert_eq!(
            preprocessing.defines,
            &[
                ("DEBUG".to_string(), None),
                ("VERSION".to_string(), Some("2".to_string()))
            ]
        );

        let binding = cfg.binding_surface();
        assert_eq!(binding.entry_headers, &[PathBuf::from("api.h")]);

        let linking = cfg.linking();
        assert_eq!(
            linking.framework_dirs,
            &[PathBuf::from("/System/Library/Frameworks")]
        );
        assert_eq!(linking.library_dirs, &[PathBuf::from("/usr/lib")]);
        assert_eq!(linking.link_libraries.len(), 1);
        assert_eq!(linking.link_frameworks.len(), 1);
        assert_eq!(linking.link_artifacts.len(), 1);
        assert_eq!(linking.platform_constraints, &["linux".to_string()]);

        let probing = cfg.probing();
        assert_eq!(probing.probe_types, &["size_t".to_string()]);
        assert!(cfg.filtering().is_some());
    }

    #[test]
    fn config_summary_distinguishes_bootstrap_domains_without_flattening_them() {
        let cfg = HeaderConfig::new()
            .entry_header("api.h")
            .add_include_dir("/usr/include")
            .define_flag("DEBUG")
            .link_library("z")
            .link_framework("Security")
            .request_probe_type_layout("size_t")
            .prefer_dynamic_linking();

        let summary = cfg.summary();
        assert_eq!(summary.entry_header_count, 1);
        assert_eq!(summary.include_dir_count, 1);
        assert_eq!(summary.define_count, 1);
        assert_eq!(summary.declared_link_input_count, 2);
        assert_eq!(summary.probe_type_count, 1);
        assert_eq!(summary.native_surface_kind, NativeSurfaceKind::LibraryNames);
        assert_eq!(summary.preferred_link_mode, LinkResolutionMode::PreferDynamic);
        assert!(summary.has_origin_filter);
    }

    #[test]
    fn config_validation_rejects_empty_values() {
        let bad_header = HeaderConfig::new().header("");
        let err = bad_header.validate().unwrap_err();
        assert!(matches!(err, LincError::InvalidConfig { .. }));
        assert!(err.to_string().contains("entry header path"));

        let bad_define = HeaderConfig::new().header("api.h").define("", None);
        let err = bad_define.validate().unwrap_err();
        assert!(matches!(err, LincError::InvalidConfig { .. }));
        assert!(err.to_string().contains("define name"));

        let bad_probe = HeaderConfig::new().header("api.h").probe_type_layout("");
        let err = bad_probe.validate().unwrap_err();
        assert!(matches!(err, LincError::InvalidConfig { .. }));
        assert!(err.to_string().contains("probe type name"));
    }

    #[test]
    fn combined_source_generation() {
        let cfg = HeaderConfig::new().header("a.h").header("b.h");
        let combined = cfg.build_combined_source();
        assert!(combined.contains("#include \"a.h\""));
        assert!(combined.contains("#include \"b.h\""));
    }

    #[test]
    fn binding_metadata_from_config_keeps_scan_and_link_inputs() {
        let cfg = HeaderConfig::new()
            .header("api.h")
            .include_dir("include")
            .framework_dir("frameworks")
            .library_dir("lib")
            .define("FOO", Some("1".into()))
            .link_framework("CoreFoundation")
            .link_static_lib("crypto")
            .link_static_artifact("lib/libcrypto.a")
            .prefer_static_linking()
            .target_constraint("linux")
            .target_constraint("x86_64")
            .probe_type_layout("struct widget");

        let target = cfg.binding_target();
        let inputs = cfg.binding_inputs();
        let link = cfg.binding_link_surface();

        assert_eq!(target.compiler_command.as_deref(), Some("gcc"));
        assert_eq!(target.flavor.as_deref(), Some("gnu-c11"));
        assert_eq!(inputs.entry_headers, vec!["api.h".to_string()]);
        assert_eq!(inputs.include_dirs, vec!["include".to_string()]);
        assert_eq!(inputs.defines.len(), 1);
        assert_eq!(link.include_paths, vec!["include".to_string()]);
        assert_eq!(link.framework_paths, vec!["frameworks".to_string()]);
        assert_eq!(link.library_paths, vec!["lib".to_string()]);
        assert_eq!(link.preferred_mode, LinkResolutionMode::PreferStatic);
        assert_eq!(link.native_surface_kind, NativeSurfaceKind::Mixed);
        assert_eq!(
            link.platform_constraints,
            vec!["linux".to_string(), "x86_64".to_string()]
        );
        assert_eq!(link.frameworks.len(), 1);
        assert_eq!(link.frameworks[0].name, "CoreFoundation");
        assert_eq!(link.libraries.len(), 1);
        assert_eq!(link.libraries[0].name, "crypto");
        assert_eq!(link.libraries[0].kind, LinkLibraryKind::Static);
        assert_eq!(link.libraries[0].source, LinkRequirementSource::Declared);
        assert_eq!(link.artifacts.len(), 1);
        assert_eq!(link.artifacts[0].path, "lib/libcrypto.a");
        assert_eq!(link.artifacts[0].kind, LinkArtifactKind::StaticLibrary);
        assert_eq!(link.artifacts[0].source, LinkRequirementSource::Declared);
        assert_eq!(link.ordered_inputs.len(), 3);
        match &link.ordered_inputs[0] {
            LinkInput::Framework(framework) => assert_eq!(framework.name, "CoreFoundation"),
            other => panic!(
                "expected first ordered input to be framework, got {:?}",
                other
            ),
        }
        match &link.ordered_inputs[1] {
            LinkInput::Library(lib) => assert_eq!(lib.name, "crypto"),
            other => panic!(
                "expected second ordered input to be library, got {:?}",
                other
            ),
        }
        match &link.ordered_inputs[2] {
            LinkInput::Artifact(artifact) => assert_eq!(artifact.path, "lib/libcrypto.a"),
            other => panic!(
                "expected third ordered input to be artifact, got {:?}",
                other
            ),
        }
        assert_eq!(cfg.probe_types, vec!["struct widget".to_string()]);
    }

    #[test]
    fn native_surface_kind_inference() {
        let header_only = HeaderConfig::new().header("api.h");
        let library_names = HeaderConfig::new().header("api.h").link_lib("m");
        let concrete = HeaderConfig::new()
            .header("api.h")
            .link_static_artifact("lib/libdemo.a");
        let mixed = HeaderConfig::new()
            .header("api.h")
            .link_lib("m")
            .link_static_artifact("lib/libdemo.a");

        assert_eq!(
            header_only.native_surface_kind(),
            NativeSurfaceKind::HeaderOnly
        );
        assert_eq!(
            library_names.native_surface_kind(),
            NativeSurfaceKind::LibraryNames
        );
        assert_eq!(
            concrete.native_surface_kind(),
            NativeSurfaceKind::ConcreteArtifacts
        );
        assert_eq!(mixed.native_surface_kind(), NativeSurfaceKind::Mixed);
    }

    #[test]
    fn parse_macro_definitions_captures_object_and_function_like_macros() {
        let macros = parse_macro_definitions(
            r#"
#define API_LEVEL 7
#define API_NAME "demo"
#define API_EXPR (1 << 2)
#define HAVE_ZLIB 1
#define API_EXPORT __attribute__((visibility("default")))
#define LOG(fmt) fmt
"#,
        );

        assert!(macros.iter().any(|m| {
            m.name == "API_LEVEL"
                && !m.function_like
                && m.form == MacroForm::ObjectLike
                && m.kind == MacroKind::Integer
                && m.category == MacroCategory::BindableConstant
                && m.value == Some(MacroValue::Integer(7))
        }));
        assert!(macros.iter().any(|m| {
            m.name == "API_NAME"
                && !m.function_like
                && m.form == MacroForm::ObjectLike
                && m.kind == MacroKind::String
                && m.category == MacroCategory::BindableConstant
                && m.value == Some(MacroValue::String("demo".into()))
        }));
        assert!(macros.iter().any(|m| {
            m.name == "API_EXPR"
                && !m.function_like
                && m.kind == MacroKind::Expression
                && m.category == MacroCategory::BindableConstant
                && m.value.is_none()
        }));
        assert!(macros.iter().any(|m| {
            m.name == "HAVE_ZLIB"
                && !m.function_like
                && m.form == MacroForm::ObjectLike
                && m.category == MacroCategory::ConfigurationFlag
                && m.value == Some(MacroValue::Integer(1))
        }));
        assert!(macros.iter().any(|m| {
            m.name == "API_EXPORT"
                && m.form == MacroForm::ObjectLike
                && m.category == MacroCategory::AbiAffecting
                && m.value.is_none()
        }));
        assert!(macros.iter().any(|m| {
            m.name == "LOG"
                && m.function_like
                && m.form == MacroForm::FunctionLike
                && m.kind == MacroKind::Other
                && m.category == MacroCategory::Unsupported
                && m.value.is_none()
        }));
    }

    #[test]
    fn parse_macro_regression_fixture_preserves_real_library_style_macros() {
        let macros = parse_macro_definitions(include_str!(
            "../tests/contracts/macro_regression_fixture.txt"
        ));

        assert!(macros.iter().any(|m| {
            m.name == "ZLIB_VERSION"
                && m.category == MacroCategory::BindableConstant
                && m.value == Some(MacroValue::String("1.3.1".into()))
        }));
        assert!(macros.iter().any(|m| {
            m.name == "PNG_LIBPNG_VER_STRING"
                && m.category == MacroCategory::BindableConstant
                && m.value == Some(MacroValue::String("1.6.43".into()))
        }));
        assert!(macros.iter().any(|m| {
            m.name == "PNG_SETJMP_SUPPORTED"
                && m.category == MacroCategory::ConfigurationFlag
                && m.value == Some(MacroValue::Integer(1))
        }));
        assert!(macros.iter().any(|m| {
            m.name == "PNGAPI"
                && m.category == MacroCategory::AbiAffecting
                && m.form == MacroForm::ObjectLike
        }));
        assert!(macros.iter().any(|m| {
            m.name == "PNG_UNUSED"
                && m.category == MacroCategory::Unsupported
                && m.form == MacroForm::FunctionLike
        }));
    }

    #[test]
    fn parse_macro_definitions_with_provenance_tracks_locations() {
        let source = concat!(
            "# 3 \"demo.h\"\n",
            "#define API_LEVEL 7\n",
            "#define API_NAME \"demo\"\n",
        );
        let (macros, provenance) = parse_macro_definitions_with_provenance(source, &["demo.h"]);
        assert_eq!(macros.len(), 2);
        assert_eq!(provenance.len(), 2);
        assert_eq!(provenance[0].macro_name, "API_LEVEL");
        assert_eq!(
            provenance[0].source_origin,
            Some(crate::line_markers::SourceOrigin::Entry)
        );
        assert_eq!(
            provenance[0]
                .source_location
                .as_ref()
                .and_then(|loc| loc.line),
            Some(3)
        );
    }

}
