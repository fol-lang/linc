use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::error::BicError;
use crate::extract::Extractor;
use crate::ir::{
    BindingDefine, BindingInputs, BindingLinkSurface, BindingPackage, BindingTarget, LinkArtifact,
    LinkArtifactKind, LinkFramework, LinkInput, LinkLibrary, LinkLibraryKind,
    LinkRequirementSource, LinkResolutionMode, MacroBinding, MacroCategory, MacroKind,
    NativeSurfaceKind,
};
use crate::line_markers::{FileOriginMap, OriginFilter};
use crate::probe::probe_type_layouts;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flavor {
    GnuC11,
    ClangC11,
    StdC11,
}

impl Flavor {
    fn to_pac(self) -> pac::driver::Flavor {
        match self {
            Flavor::GnuC11 => pac::driver::Flavor::GnuC11,
            Flavor::ClangC11 => pac::driver::Flavor::ClangC11,
            Flavor::StdC11 => pac::driver::Flavor::StdC11,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingReport {
    pub command: String,
    pub args: Vec<String>,
    pub preprocessed_source: String,
}

#[derive(Debug)]
pub struct RawHeaderResult {
    pub package: BindingPackage,
    pub report: PreprocessingReport,
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

    pub fn include_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.include_dirs.push(path.into());
        self
    }

    pub fn framework_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.framework_dirs.push(path.into());
        self
    }

    pub fn library_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.library_dirs.push(path.into());
        self
    }

    pub fn define(mut self, name: impl Into<String>, value: Option<String>) -> Self {
        self.defines.push((name.into(), value));
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

    pub fn probe_type_layout(mut self, name: impl Into<String>) -> Self {
        self.probe_types.push(name.into());
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

    pub fn process(&self) -> Result<RawHeaderResult, BicError> {
        if self.entry_headers.is_empty() {
            return Err(BicError::NoHeaders);
        }

        // Build a combined header source that includes all entry headers
        let combined = self.build_combined_source();
        let unique_id = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_dir = std::env::temp_dir().join(format!("bic_raw_{unique_id}_{ts}"));
        std::fs::create_dir_all(&tmp_dir)?;
        let tmp_file = tmp_dir.join("_bic_combined.c");
        std::fs::write(&tmp_file, &combined)?;

        let pac_config = self.build_pac_config();
        let (command, args) = self.describe_invocation(&pac_config, &tmp_file);

        let parse_result = pac::driver::parse(&pac_config, &tmp_file);
        let macros = self.capture_macros(&tmp_file);

        // Clean up
        std::fs::remove_file(&tmp_file).ok();
        std::fs::remove_dir(&tmp_dir).ok();

        match parse_result {
            Ok(parsed) => {
                let report = PreprocessingReport {
                    command,
                    args,
                    preprocessed_source: parsed.source.clone(),
                };

                let extractor = Extractor::new();
                let (items, diagnostics) = extractor.extract(&parsed.unit);

                let source_desc = self
                    .entry_headers
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                let mut package = BindingPackage {
                    source_path: Some(source_desc),
                    target: self.binding_target(),
                    inputs: self.binding_inputs(),
                    macros,
                    link: self.binding_link_surface(),
                    items,
                    diagnostics,
                    ..BindingPackage::new()
                };

                if !self.probe_types.is_empty() {
                    package.layouts = probe_type_layouts(self, &self.probe_types)
                        .map_err(|reason| BicError::ProbeFailed { reason })?
                        .layouts;
                }

                // Apply origin filtering if configured
                if let Some(ref filter) = self.origin_filter {
                    let origin_map =
                        FileOriginMap::parse(&parsed.source, &self.entry_headers);
                    package.filter_by_origin(&origin_map, filter);
                }

                Ok(RawHeaderResult { package, report })
            }
            Err(pac::driver::Error::PreprocessorError(e)) => {
                let mut pkg = BindingPackage::new();
                pkg.diagnostics.push(
                    Diagnostic::error(
                        DiagnosticKind::PreprocessingFailed,
                        format!("preprocessor failed: {}", e),
                    ),
                );
                Ok(RawHeaderResult {
                    package: pkg,
                    report: PreprocessingReport {
                        command,
                        args,
                        preprocessed_source: String::new(),
                    },
                })
            }
            Err(pac::driver::Error::SyntaxError(e)) => {
                let mut pkg = BindingPackage::new();
                pkg.diagnostics.push(
                    Diagnostic::error(
                        DiagnosticKind::ParseFailed,
                        format!("parse error: {}", e),
                    ),
                );
                Ok(RawHeaderResult {
                    package: pkg,
                    report: PreprocessingReport {
                        command,
                        args,
                        preprocessed_source: e.source,
                    },
                })
            }
        }
    }

    fn build_combined_source(&self) -> String {
        let mut source = String::new();
        for header in &self.entry_headers {
            source.push_str(&format!("#include \"{}\"\n", header.display()));
        }
        source
    }

    fn build_pac_config(&self) -> pac::driver::Config {
        let flavor = self.flavor.unwrap_or(Flavor::GnuC11);
        let compiler = self
            .compiler
            .clone()
            .unwrap_or_else(|| match flavor {
                Flavor::ClangC11 => "clang".into(),
                _ => "gcc".into(),
            });

        let mut cpp_options = vec!["-E".to_string()];

        for dir in &self.include_dirs {
            cpp_options.push(format!("-I{}", dir.display()));
        }

        for (name, value) in &self.defines {
            match value {
                Some(v) => cpp_options.push(format!("-D{}={}", name, v)),
                None => cpp_options.push(format!("-D{}", name)),
            }
        }

        pac::driver::Config {
            cpp_command: compiler,
            cpp_options,
            flavor: flavor.to_pac(),
        }
    }

    fn binding_target(&self) -> BindingTarget {
        let compiler_command = self.compiler_command();
        BindingTarget {
            target_triple: detect_target_triple(&compiler_command),
            compiler_command: Some(compiler_command.clone()),
            compiler_version: detect_compiler_version(&compiler_command),
            flavor: Some(self.flavor_label()),
        }
    }

    fn binding_inputs(&self) -> BindingInputs {
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

    fn binding_link_surface(&self) -> BindingLinkSurface {
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

    fn native_surface_kind(&self) -> NativeSurfaceKind {
        match (self.link_libraries.is_empty() && self.link_frameworks.is_empty(), self.link_artifacts.is_empty()) {
            (true, true) => NativeSurfaceKind::HeaderOnly,
            (false, true) => NativeSurfaceKind::LibraryNames,
            (true, false) => NativeSurfaceKind::ConcreteArtifacts,
            (false, false) => NativeSurfaceKind::Mixed,
        }
    }

    fn capture_macros(&self, input: &Path) -> Vec<MacroBinding> {
        let compiler = self.compiler_command();
        let mut cmd = std::process::Command::new(&compiler);
        cmd.arg("-dM").arg("-E");
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
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return Vec::new();
        };
        parse_macro_definitions(&stdout)
    }

    fn describe_invocation(&self, config: &pac::driver::Config, input: &Path) -> (String, Vec<String>) {
        let command = config.cpp_command.clone();
        let mut args = config.cpp_options.clone();
        args.push(input.display().to_string());
        (command, args)
    }

    fn compiler_command(&self) -> String {
        let flavor = self.flavor.unwrap_or(Flavor::GnuC11);
        self.compiler
            .clone()
            .unwrap_or_else(|| match flavor {
                Flavor::ClangC11 => "clang".into(),
                _ => "gcc".into(),
            })
    }

    fn flavor_label(&self) -> String {
        match self.flavor.unwrap_or(Flavor::GnuC11) {
            Flavor::GnuC11 => "gnu-c11".into(),
            Flavor::ClangC11 => "clang-c11".into(),
            Flavor::StdC11 => "std-c11".into(),
        }
    }
}

fn detect_target_triple(compiler_command: &str) -> Option<String> {
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

fn detect_compiler_version(compiler_command: &str) -> Option<String> {
    let output = std::process::Command::new(compiler_command)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout.lines().next().map(str::to_string).filter(|line| !line.is_empty())
}

fn parse_macro_definitions(source: &str) -> Vec<MacroBinding> {
    source
        .lines()
        .filter_map(parse_macro_definition_line)
        .collect()
}

fn parse_macro_definition_line(line: &str) -> Option<MacroBinding> {
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
        name,
        body,
        function_like,
    })
}

fn classify_macro_body(body: &str, function_like: bool) -> MacroKind {
    if function_like {
        return MacroKind::Other;
    }

    if body.starts_with('"') && body.ends_with('"') && body.len() >= 2 {
        return MacroKind::String;
    }

    let trimmed = body.trim();
    if !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() || matches!(ch, 'x' | 'X' | 'u' | 'U' | 'l' | 'L' | '+' | '-'))
    {
        return MacroKind::Integer;
    }

    if trimmed.chars().any(|ch| "+-*/%<>&|^!()".contains(ch)) {
        return MacroKind::Expression;
    }

    MacroKind::Other
}

fn classify_macro_category(name: &str, body: &str, function_like: bool) -> MacroCategory {
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

    fn setup_test_dir(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bic_raw_{}_{}", name, id));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        std::fs::remove_dir_all(dir).ok();
    }

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
        let result = cfg.process();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BicError::NoHeaders));
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
    fn combined_source_generation() {
        let cfg = HeaderConfig::new()
            .header("a.h")
            .header("b.h");
        let combined = cfg.build_combined_source();
        assert!(combined.contains("#include \"a.h\""));
        assert!(combined.contains("#include \"b.h\""));
    }

    #[test]
    fn pac_config_includes_and_defines() {
        let cfg = HeaderConfig::new()
            .header("test.h")
            .include_dir("/inc")
            .define("DEBUG", None)
            .define("VER", Some("3".into()));

        let pac_cfg = cfg.build_pac_config();
        assert!(pac_cfg.cpp_options.contains(&"-I/inc".to_string()));
        assert!(pac_cfg.cpp_options.contains(&"-DDEBUG".to_string()));
        assert!(pac_cfg.cpp_options.contains(&"-DVER=3".to_string()));
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
            other => panic!("expected first ordered input to be framework, got {:?}", other),
        }
        match &link.ordered_inputs[1] {
            LinkInput::Library(lib) => assert_eq!(lib.name, "crypto"),
            other => panic!("expected second ordered input to be library, got {:?}", other),
        }
        match &link.ordered_inputs[2] {
            LinkInput::Artifact(artifact) => assert_eq!(artifact.path, "lib/libcrypto.a"),
            other => panic!("expected third ordered input to be artifact, got {:?}", other),
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

        assert_eq!(header_only.native_surface_kind(), NativeSurfaceKind::HeaderOnly);
        assert_eq!(library_names.native_surface_kind(), NativeSurfaceKind::LibraryNames);
        assert_eq!(concrete.native_surface_kind(), NativeSurfaceKind::ConcreteArtifacts);
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
                && m.kind == MacroKind::Integer
                && m.category == MacroCategory::BindableConstant
        }));
        assert!(macros.iter().any(|m| {
            m.name == "API_NAME"
                && !m.function_like
                && m.kind == MacroKind::String
                && m.category == MacroCategory::BindableConstant
        }));
        assert!(macros.iter().any(|m| {
            m.name == "API_EXPR"
                && !m.function_like
                && m.kind == MacroKind::Expression
                && m.category == MacroCategory::BindableConstant
        }));
        assert!(macros.iter().any(|m| {
            m.name == "HAVE_ZLIB"
                && !m.function_like
                && m.category == MacroCategory::ConfigurationFlag
        }));
        assert!(macros.iter().any(|m| {
            m.name == "API_EXPORT" && m.category == MacroCategory::AbiAffecting
        }));
        assert!(macros.iter().any(|m| {
            m.name == "LOG"
                && m.function_like
                && m.kind == MacroKind::Other
                && m.category == MacroCategory::Unsupported
        }));
    }

    #[test]
    fn flavor_to_pac_conversion() {
        assert_eq!(Flavor::GnuC11.to_pac(), pac::driver::Flavor::GnuC11);
        assert_eq!(Flavor::ClangC11.to_pac(), pac::driver::Flavor::ClangC11);
        assert_eq!(Flavor::StdC11.to_pac(), pac::driver::Flavor::StdC11);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_single_header() {
        let dir = setup_test_dir("t");
        let header = dir.join("simple.h");
        std::fs::write(&header, "int add(int a, int b);\n").unwrap();

        let result = HeaderConfig::new()
            .header(&header)
            .process()
            .unwrap();

        assert!(!result.report.command.is_empty());
        assert!(!result.report.preprocessed_source.is_empty());
        assert_eq!(result.package.inputs.entry_headers.len(), 1);
        assert_eq!(result.package.target.compiler_command.as_deref(), Some("gcc"));

        let funcs: Vec<_> = result.package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f),
            _ => None,
        }).collect();
        assert!(funcs.iter().any(|f| f.name == "add"));

        cleanup(&dir);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_with_include_dir() {
        let dir = setup_test_dir("t");
        let inc = dir.join("inc");
        std::fs::create_dir_all(&inc).unwrap();

        std::fs::write(inc.join("types.h"), "typedef unsigned long mysize_t;\n").unwrap();
        let header = dir.join("api.h");
        std::fs::write(&header, "#include \"types.h\"\nmysize_t get_size(void);\n").unwrap();

        let result = HeaderConfig::new()
            .header(&header)
            .include_dir(&inc)
            .process()
            .unwrap();

        assert!(result.package.diagnostics.is_empty()
            || result.package.diagnostics.iter().all(|d| d.severity == crate::diagnostics::Severity::Warning));

        let funcs: Vec<_> = result.package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f),
            _ => None,
        }).collect();
        assert!(funcs.iter().any(|f| f.name == "get_size"));

        cleanup(&dir);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_with_defines() {
        let dir = setup_test_dir("t");
        let header = dir.join("cond.h");
        std::fs::write(
            &header,
            r#"
#ifdef USE_FLOAT
float compute(float x);
#else
int compute(int x);
#endif
"#,
        )
        .unwrap();

        let result = HeaderConfig::new()
            .header(&header)
            .define("USE_FLOAT", None)
            .process()
            .unwrap();

        let funcs: Vec<_> = result.package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f),
            _ => None,
        }).collect();

        let compute = funcs.iter().find(|f| f.name == "compute").unwrap();
        assert_eq!(compute.return_type, BindingType::Float);

        cleanup(&dir);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_multiple_headers() {
        let dir = setup_test_dir("t");
        let h1 = dir.join("a.h");
        let h2 = dir.join("b.h");
        std::fs::write(&h1, "void func_a(void);\n").unwrap();
        std::fs::write(&h2, "void func_b(void);\n").unwrap();

        let result = HeaderConfig::new()
            .header(&h1)
            .header(&h2)
            .process()
            .unwrap();

        let names: Vec<_> = result.package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        }).collect();
        assert!(names.contains(&"func_a"));
        assert!(names.contains(&"func_b"));

        cleanup(&dir);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_nonexistent_header() {
        let result = HeaderConfig::new()
            .header("/nonexistent/path.h")
            .process()
            .unwrap();

        assert!(!result.package.diagnostics.is_empty());
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn report_captures_metadata() {
        let dir = setup_test_dir("t");
        let header = dir.join("meta.h");
        std::fs::write(&header, "void noop(void);\n").unwrap();

        let result = HeaderConfig::new()
            .header(&header)
            .include_dir("/some/path")
            .define("FOO", Some("1".into()))
            .process()
            .unwrap();

        assert!(result.report.args.iter().any(|a| a.contains("-I/some/path")));
        assert!(result.report.args.iter().any(|a| a.contains("-DFOO=1")));

        cleanup(&dir);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_captures_header_macros() {
        let dir = setup_test_dir("t");
        let header = dir.join("macros.h");
        std::fs::write(
            &header,
            "#define API_LEVEL 7\n#define API_NAME \"demo\"\n#define HAVE_ZLIB 1\nint noop(void);\n",
        )
        .unwrap();

        let result = HeaderConfig::new().header(&header).process().unwrap();

        assert!(result
            .package
            .macros
            .iter()
            .any(|m| m.name == "API_LEVEL"
                && m.kind == MacroKind::Integer
                && m.category == MacroCategory::BindableConstant));
        assert!(result
            .package
            .macros
            .iter()
            .any(|m| m.name == "API_NAME"
                && m.kind == MacroKind::String
                && m.category == MacroCategory::BindableConstant));
        assert!(result
            .package
            .macros
            .iter()
            .any(|m| m.name == "HAVE_ZLIB"
                && m.category == MacroCategory::ConfigurationFlag));

        cleanup(&dir);
    }

    #[test]
    #[ignore] // Requires gcc/clang
    fn process_attaches_requested_type_layouts() {
        let dir = setup_test_dir("t");
        let header = dir.join("layout.h");
        std::fs::write(
            &header,
            "typedef unsigned int value_t;\nstruct widget { int a; double b; };\n",
        )
        .unwrap();

        let result = HeaderConfig::new()
            .header(&header)
            .probe_type_layout("value_t")
            .probe_type_layout("struct widget")
            .process()
            .unwrap();

        assert!(result
            .package
            .layouts
            .iter()
            .any(|layout| layout.name == "value_t"));
        assert!(result
            .package
            .layouts
            .iter()
            .any(|layout| layout.name == "struct widget"));

        cleanup(&dir);
    }

    /// Test that origin filtering removes system header declarations.
    #[test]
    #[ignore] // Requires gcc/clang
    fn origin_filter_removes_system_headers() {
        let dir = setup_test_dir("t");
        let header = dir.join("mylib.h");
        // Include stdio.h (system header) and define our own function
        std::fs::write(
            &header,
            "#include <stdio.h>\nint my_func(int x);\n",
        )
        .unwrap();

        // With default filter (exclude system)
        let filtered = HeaderConfig::new()
            .header(&header)
            .process()
            .unwrap();

        let filtered_names: Vec<_> = filtered.package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        }).collect();

        // my_func should be present, printf should be filtered out
        assert!(filtered_names.contains(&"my_func"));
        assert!(!filtered_names.contains(&"printf"), "system functions should be filtered");

        // Without filter — should include system declarations
        let unfiltered = HeaderConfig::new()
            .header(&header)
            .no_origin_filter()
            .process()
            .unwrap();

        let unfiltered_names: Vec<_> = unfiltered.package.items.iter().filter_map(|i| match i {
            BindingItem::Function(f) => Some(f.name.as_str()),
            _ => None,
        }).collect();

        // Both should be present without filtering
        assert!(unfiltered_names.contains(&"my_func"));
        // System header functions should now appear
        assert!(unfiltered_names.len() > filtered_names.len(),
            "unfiltered should have more items than filtered");

        cleanup(&dir);
    }
}
