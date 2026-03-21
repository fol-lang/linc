use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;
use crate::ir::{BindingInputs, BindingLinkSurface, BindingTarget, BindingPackage, SCHEMA_VERSION};
use crate::link_plan::{resolve_link_plan, ResolvedLinkPlan};
use crate::probe::AbiProbeReport;

#[cfg(feature = "symbols")]
use crate::symbols::SymbolInventory;
#[cfg(feature = "symbols")]
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
    pub resolved_link_plan: Option<ResolvedLinkPlan>,
    #[serde(default)]
    pub abi_probe: Option<AbiProbeReport>,
    #[cfg(feature = "symbols")]
    #[serde(default)]
    pub symbol_inventories: Vec<SymbolInventory>,
    #[cfg(feature = "symbols")]
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
            resolved_link_plan: None,
            abi_probe: None,
            #[cfg(feature = "symbols")]
            symbol_inventories: Vec::new(),
            #[cfg(feature = "symbols")]
            validation: None,
        }
    }
}

impl LinkAnalysisPackage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a link-analysis contract from the legacy all-in-one binding package.
    pub fn from_binding_package(package: &BindingPackage) -> Self {
        Self {
            schema_version: schema_version(),
            linc_version: linc_version(),
            target: package.target.clone(),
            inputs: package.inputs.clone(),
            diagnostics: package.diagnostics.clone(),
            declared_link_surface: package.link.clone(),
            resolved_link_plan: Some(resolve_link_plan(package)),
            abi_probe: None,
            #[cfg(feature = "symbols")]
            symbol_inventories: Vec::new(),
            #[cfg(feature = "symbols")]
            validation: None,
        }
    }
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
        BindingItem, FunctionBinding, LinkArtifact, LinkArtifactKind, LinkInput, LinkLibrary,
        LinkLibraryKind, LinkRequirementSource, ParameterBinding, CallingConvention, BindingType,
    };

    fn sample_binding_package() -> BindingPackage {
        let mut package = BindingPackage::new();
        package
            .items
            .push(BindingItem::Function(FunctionBinding {
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
        package.link.ordered_inputs.push(LinkInput::Library(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));
        package.link.ordered_inputs.push(LinkInput::Artifact(LinkArtifact {
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
    }

    #[test]
    fn analysis_package_from_binding_package_carries_link_contract() {
        let package = sample_binding_package();
        let analysis = LinkAnalysisPackage::from_binding_package(&package);

        assert_eq!(analysis.inputs, package.inputs);
        assert_eq!(analysis.target, package.target);
        assert_eq!(analysis.declared_link_surface, package.link);
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
}
