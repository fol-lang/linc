use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;
use crate::ir::LinkInput;
use crate::ir::{BindingInputs, BindingLinkSurface, BindingPackage, BindingTarget, SCHEMA_VERSION};
use crate::link_plan::{resolve_link_plan, ResolvedLinkPlan};
use crate::probe::AbiProbeReport;

use crate::symbols::SymbolInventory;
use crate::validate::ValidationReport;

/// Frontend-agnostic link/binary evidence contract produced by LINC.
///
/// This package is intentionally narrower than `BindingPackage`: it carries
/// link and binary analysis state, not source declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkAnalysisPackage {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default = "linc_version")]
    pub linc_version: String,
    #[serde(default)]
    pub target: BindingTarget,
    #[serde(default)]
    pub inputs: BindingInputs,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub declared_link_surface: BindingLinkSurface,
    #[serde(default)]
    pub runtime_boundaries: Vec<RuntimeBoundary>,
    #[serde(default)]
    pub resolved_link_plan: Option<ResolvedLinkPlan>,
    #[serde(default)]
    pub abi_probe: Option<AbiProbeReport>,
    #[serde(default)]
    pub symbol_inventories: Vec<SymbolInventory>,
    #[serde(default)]
    pub validation: Option<ValidationReport>,
}

impl Default for LinkAnalysisPackage {
    fn default() -> Self {
        Self {
            schema_version: schema_version(),
            linc_version: linc_version(),
            target: BindingTarget::default(),
            inputs: BindingInputs::default(),
            diagnostics: Vec::new(),
            declared_link_surface: BindingLinkSurface::default(),
            runtime_boundaries: Vec::new(),
            resolved_link_plan: None,
            abi_probe: None,
            symbol_inventories: Vec::new(),
            validation: None,
        }
    }
}

impl LinkAnalysisPackage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a link-analysis contract from LINC's current internal binding IR.
    pub fn from_binding_package(package: &BindingPackage) -> Self {
        Self {
            schema_version: schema_version(),
            linc_version: linc_version(),
            target: package.target.clone(),
            inputs: package.inputs.clone(),
            diagnostics: package.diagnostics.clone(),
            declared_link_surface: package.link.clone(),
            runtime_boundaries: detect_runtime_boundaries(package),
            resolved_link_plan: Some(resolve_link_plan(package)),
            abi_probe: None,
            symbol_inventories: Vec::new(),
            validation: None,
        }
    }
}

impl From<&BindingPackage> for LinkAnalysisPackage {
    fn from(package: &BindingPackage) -> Self {
        Self::from_binding_package(package)
    }
}

impl From<BindingPackage> for LinkAnalysisPackage {
    fn from(package: BindingPackage) -> Self {
        Self::from_binding_package(&package)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeBoundaryKind {
    DynamicLoader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBoundary {
    pub kind: RuntimeBoundaryKind,
    pub trigger: String,
    pub message: String,
}

fn detect_runtime_boundaries(package: &BindingPackage) -> Vec<RuntimeBoundary> {
    let mut boundaries = Vec::new();
    for input in &package.link.ordered_inputs {
        if let LinkInput::Library(library) = input {
            if library.name == "dl" {
                boundaries.push(RuntimeBoundary {
                    kind: RuntimeBoundaryKind::DynamicLoader,
                    trigger: library.name.clone(),
                    message:
                        "declared dynamic-loader dependency requires downstream runtime policy"
                            .into(),
                });
            }
        }
    }
    boundaries
}

const fn schema_version() -> u32 {
    SCHEMA_VERSION
}

fn linc_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BindingItem, BindingType, CallingConvention, FunctionBinding, LinkArtifact,
        LinkArtifactKind, LinkInput, LinkLibrary, LinkLibraryKind, LinkRequirementSource,
        ParameterBinding,
    };

    fn sample_binding_package() -> BindingPackage {
        let mut package = BindingPackage::new();
        package.items.push(BindingItem::Function(FunctionBinding {
            name: "demo_open".into(),
            calling_convention: CallingConvention::C,
            parameters: vec![ParameterBinding {
                name: Some("flags".into()),
                ty: BindingType::Int,
            }],
            return_type: BindingType::Int,
            variadic: false,
            source_offset: None,
        }));
        package
            .link
            .ordered_inputs
            .push(LinkInput::Library(LinkLibrary {
                name: "demo".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            }));
        package
            .link
            .ordered_inputs
            .push(LinkInput::Artifact(LinkArtifact {
                path: "/tmp/libdemo.so".into(),
                kind: LinkArtifactKind::SharedLibrary,
                source: LinkRequirementSource::Discovered,
            }));
        package
    }

    #[test]
    fn default_analysis_package_is_empty() {
        let analysis = LinkAnalysisPackage::new();
        assert_eq!(analysis.schema_version, SCHEMA_VERSION);
        assert!(analysis.resolved_link_plan.is_none());
        assert!(analysis.diagnostics.is_empty());
        assert!(analysis.declared_link_surface.ordered_inputs.is_empty());
        assert!(analysis.runtime_boundaries.is_empty());
    }

    #[test]
    fn analysis_package_from_binding_package_carries_link_contract() {
        let package = sample_binding_package();
        let analysis = LinkAnalysisPackage::from_binding_package(&package);

        assert_eq!(analysis.inputs, package.inputs);
        assert_eq!(analysis.target, package.target);
        assert_eq!(analysis.declared_link_surface, package.link);
        assert!(analysis.runtime_boundaries.is_empty());
        assert_eq!(
            analysis
                .resolved_link_plan
                .as_ref()
                .expect("resolved plan")
                .inputs,
            package.link.ordered_inputs
        );
    }

    #[test]
    fn analysis_package_json_roundtrip() {
        let package = sample_binding_package();
        let analysis = LinkAnalysisPackage::from_binding_package(&package);

        let json = serde_json::to_string_pretty(&analysis).unwrap();
        let decoded: LinkAnalysisPackage = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, analysis);
    }

    #[test]
    fn analysis_package_marks_dynamic_loader_boundaries_explicitly() {
        let mut package = sample_binding_package();
        package
            .link
            .ordered_inputs
            .push(LinkInput::Library(LinkLibrary {
                name: "dl".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            }));

        let analysis = LinkAnalysisPackage::from_binding_package(&package);

        assert_eq!(analysis.runtime_boundaries.len(), 1);
        assert_eq!(
            analysis.runtime_boundaries[0].kind,
            RuntimeBoundaryKind::DynamicLoader
        );
        assert_eq!(analysis.runtime_boundaries[0].trigger, "dl");
    }
}
