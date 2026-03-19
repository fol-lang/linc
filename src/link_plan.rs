use serde::{Deserialize, Serialize};

use crate::ir::{BindingPackage, LinkInput, LinkResolutionMode, NativeSurfaceKind};
use crate::symbols::SymbolInventory;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderMatchKind {
    ExactArtifact,
    LibraryName,
    FrameworkName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProvider {
    pub artifact_path: String,
    pub match_kind: ProviderMatchKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLinkRequirement {
    pub declared: LinkInput,
    #[serde(default)]
    pub providers: Vec<ResolvedProvider>,
}

/// First-class library-facing link plan derived from a package link surface.
///
/// Invariant: this is currently a normalized planning artifact, not a full filesystem-resolved
/// linker invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResolvedLinkPlan {
    #[serde(default)]
    pub preferred_mode: LinkResolutionMode,
    #[serde(default)]
    pub native_surface_kind: NativeSurfaceKind,
    #[serde(default)]
    pub platform_constraints: Vec<String>,
    #[serde(default)]
    pub inputs: Vec<LinkInput>,
    #[serde(default)]
    pub requirements: Vec<ResolvedLinkRequirement>,
}

pub fn resolve_link_plan(package: &BindingPackage) -> ResolvedLinkPlan {
    resolve_link_plan_with_inventories(package, &[])
}

pub fn resolve_link_plan_with_inventories(
    package: &BindingPackage,
    inventories: &[SymbolInventory],
) -> ResolvedLinkPlan {
    let requirements = package
        .link
        .ordered_inputs
        .iter()
        .cloned()
        .map(|declared| ResolvedLinkRequirement {
            providers: matching_providers(&declared, inventories),
            declared,
        })
        .collect();

    ResolvedLinkPlan {
        preferred_mode: package.link.preferred_mode,
        native_surface_kind: package.link.native_surface_kind,
        platform_constraints: package.link.platform_constraints.clone(),
        inputs: package.link.ordered_inputs.clone(),
        requirements,
    }
}

fn matching_providers(input: &LinkInput, inventories: &[SymbolInventory]) -> Vec<ResolvedProvider> {
    inventories
        .iter()
        .filter_map(|inventory| match input {
            LinkInput::Artifact(artifact) if inventory.artifact_path == artifact.path => {
                Some(ResolvedProvider {
                    artifact_path: inventory.artifact_path.clone(),
                    match_kind: ProviderMatchKind::ExactArtifact,
                })
            }
            LinkInput::Library(library) if inventory_matches_library_name(&inventory.artifact_path, &library.name) => {
                Some(ResolvedProvider {
                    artifact_path: inventory.artifact_path.clone(),
                    match_kind: ProviderMatchKind::LibraryName,
                })
            }
            LinkInput::Framework(framework)
                if inventory_matches_framework_name(&inventory.artifact_path, &framework.name) =>
            {
                Some(ResolvedProvider {
                    artifact_path: inventory.artifact_path.clone(),
                    match_kind: ProviderMatchKind::FrameworkName,
                })
            }
            _ => None,
        })
        .collect()
}

fn inventory_matches_library_name(path: &str, name: &str) -> bool {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    [
        format!("lib{}.a", name),
        format!("lib{}.so", name),
        format!("lib{}.dylib", name),
    ]
    .iter()
    .any(|candidate| file_name == candidate)
}

fn inventory_matches_framework_name(path: &str, name: &str) -> bool {
    path.contains(&format!("{name}.framework")) || path.ends_with(&format!("/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BindingLinkSurface, LinkArtifact, LinkArtifactKind, LinkFramework, LinkLibrary,
        LinkLibraryKind, LinkRequirementSource,
    };
    use crate::symbols::{ArtifactCapabilities, ArtifactFormat, ArtifactKind, ArtifactPlatform};

    #[test]
    fn resolve_link_plan_preserves_declared_order() {
        let mut package = BindingPackage::new();
        package.link = BindingLinkSurface {
            preferred_mode: LinkResolutionMode::PreferDynamic,
            native_surface_kind: NativeSurfaceKind::Mixed,
            platform_constraints: vec!["linux".into()],
            ordered_inputs: vec![
                LinkInput::Library(LinkLibrary {
                    name: "z".into(),
                    kind: LinkLibraryKind::Default,
                    source: LinkRequirementSource::Declared,
                }),
                LinkInput::Framework(LinkFramework {
                    name: "CoreFoundation".into(),
                    source: LinkRequirementSource::Declared,
                }),
            ],
            ..BindingLinkSurface::default()
        };

        let plan = resolve_link_plan(&package);
        assert_eq!(plan.preferred_mode, LinkResolutionMode::PreferDynamic);
        assert_eq!(plan.native_surface_kind, NativeSurfaceKind::Mixed);
        assert_eq!(plan.platform_constraints, vec!["linux".to_string()]);
        assert_eq!(plan.inputs, package.link.ordered_inputs);
        assert_eq!(plan.requirements.len(), 2);
        assert!(plan.requirements.iter().all(|req| req.providers.is_empty()));
    }

    #[test]
    fn resolve_link_plan_with_inventories_separates_requirements_and_providers() {
        let mut package = BindingPackage::new();
        package.link = BindingLinkSurface {
            ordered_inputs: vec![
                LinkInput::Library(LinkLibrary {
                    name: "z".into(),
                    kind: LinkLibraryKind::Default,
                    source: LinkRequirementSource::Declared,
                }),
                LinkInput::Artifact(LinkArtifact {
                    path: "/tmp/libdemo.a".into(),
                    kind: LinkArtifactKind::StaticLibrary,
                    source: LinkRequirementSource::Declared,
                }),
            ],
            ..BindingLinkSurface::default()
        };
        let inventories = vec![
            SymbolInventory {
                artifact_path: "/usr/lib/libz.so".into(),
                format: ArtifactFormat::ElfSharedLibrary,
                platform: ArtifactPlatform::Elf,
                kind: ArtifactKind::SharedLibrary,
                capabilities: ArtifactCapabilities {
                    exports_symbols: true,
                    imports_symbols: true,
                },
                dependency_edges: Vec::new(),
                symbols: Vec::new(),
            },
            SymbolInventory {
                artifact_path: "/tmp/libdemo.a".into(),
                format: ArtifactFormat::ElfStaticLibrary,
                platform: ArtifactPlatform::Elf,
                kind: ArtifactKind::StaticLibrary,
                capabilities: ArtifactCapabilities {
                    exports_symbols: true,
                    imports_symbols: false,
                },
                dependency_edges: Vec::new(),
                symbols: Vec::new(),
            },
        ];

        let plan = resolve_link_plan_with_inventories(&package, &inventories);
        assert_eq!(plan.requirements.len(), 2);
        assert_eq!(plan.requirements[0].providers.len(), 1);
        assert_eq!(plan.requirements[0].providers[0].match_kind, ProviderMatchKind::LibraryName);
        assert_eq!(plan.requirements[1].providers.len(), 1);
        assert_eq!(
            plan.requirements[1].providers[0].match_kind,
            ProviderMatchKind::ExactArtifact
        );
    }
}
