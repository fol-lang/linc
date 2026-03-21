use serde::{Deserialize, Serialize};

use crate::ir::{
    BindingPackage, LinkInput, LinkRequirementSource, LinkResolutionMode, NativeSurfaceKind,
};
use crate::symbols::SymbolInventory;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderMatchKind {
    ExactArtifact,
    LibraryName,
    FrameworkName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderProvenance {
    DeclaredArtifact,
    DiscoveredInventory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProvider {
    pub artifact_path: String,
    pub match_kind: ProviderMatchKind,
    pub provenance: ProviderProvenance,
    #[serde(default)]
    pub dependency_edges: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RequirementResolution {
    #[default]
    Unresolved,
    Resolved,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLinkRequirement {
    pub declared: LinkInput,
    #[serde(default)]
    pub source: LinkRequirementSource,
    #[serde(default)]
    pub resolution: RequirementResolution,
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
    #[serde(default)]
    pub transitive_dependencies: Vec<String>,
}

pub fn resolve_link_plan(package: &BindingPackage) -> ResolvedLinkPlan {
    resolve_link_plan_for_target(package, &[], None)
}

pub fn resolve_link_plan_with_inventories(
    package: &BindingPackage,
    inventories: &[SymbolInventory],
) -> ResolvedLinkPlan {
    resolve_link_plan_for_target(package, inventories, None)
}

pub fn resolve_link_plan_for_target(
    package: &BindingPackage,
    inventories: &[SymbolInventory],
    target: Option<&str>,
) -> ResolvedLinkPlan {
    let target_matches = target_matches_constraints(&package.link.platform_constraints, target);
    let requirements: Vec<ResolvedLinkRequirement> = if target_matches {
        package
            .link
            .ordered_inputs
            .iter()
            .cloned()
            .map(|declared| {
                let providers = matching_providers(&declared, inventories);
                ResolvedLinkRequirement {
                    source: declared_requirement_source(&declared),
                    resolution: match providers.len() {
                        0 => RequirementResolution::Unresolved,
                        1 => RequirementResolution::Resolved,
                        _ => RequirementResolution::Ambiguous,
                    },
                    providers,
                    declared,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let transitive_dependencies = requirements
        .iter()
        .flat_map(|requirement| requirement.providers.iter())
        .flat_map(|provider| provider.dependency_edges.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    ResolvedLinkPlan {
        preferred_mode: package.link.preferred_mode,
        native_surface_kind: package.link.native_surface_kind,
        platform_constraints: filtered_constraints(&package.link.platform_constraints, target),
        inputs: if target_matches {
            package.link.ordered_inputs.clone()
        } else {
            Vec::new()
        },
        requirements,
        transitive_dependencies,
    }
}

fn filtered_constraints(constraints: &[String], target: Option<&str>) -> Vec<String> {
    if target_matches_constraints(constraints, target) {
        constraints.to_vec()
    } else {
        Vec::new()
    }
}

fn target_matches_constraints(constraints: &[String], target: Option<&str>) -> bool {
    constraints.is_empty()
        || target.is_none()
        || target.is_some_and(|target| {
            constraints
                .iter()
                .any(|constraint| target.contains(constraint))
        })
}

fn matching_providers(input: &LinkInput, inventories: &[SymbolInventory]) -> Vec<ResolvedProvider> {
    inventories
        .iter()
        .filter_map(|inventory| match input {
            LinkInput::Artifact(artifact) if inventory.artifact_path == artifact.path => {
                Some(ResolvedProvider {
                    artifact_path: inventory.artifact_path.clone(),
                    match_kind: ProviderMatchKind::ExactArtifact,
                    provenance: ProviderProvenance::DeclaredArtifact,
                    dependency_edges: inventory.dependency_edges.clone(),
                })
            }
            LinkInput::Library(library)
                if inventory_matches_library_name(&inventory.artifact_path, &library.name) =>
            {
                Some(ResolvedProvider {
                    artifact_path: inventory.artifact_path.clone(),
                    match_kind: ProviderMatchKind::LibraryName,
                    provenance: ProviderProvenance::DiscoveredInventory,
                    dependency_edges: inventory.dependency_edges.clone(),
                })
            }
            LinkInput::Framework(framework)
                if inventory_matches_framework_name(&inventory.artifact_path, &framework.name) =>
            {
                Some(ResolvedProvider {
                    artifact_path: inventory.artifact_path.clone(),
                    match_kind: ProviderMatchKind::FrameworkName,
                    provenance: ProviderProvenance::DiscoveredInventory,
                    dependency_edges: inventory.dependency_edges.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

fn declared_requirement_source(input: &LinkInput) -> LinkRequirementSource {
    match input {
        LinkInput::Library(library) => library.source,
        LinkInput::Artifact(artifact) => artifact.source,
        LinkInput::Framework(framework) => framework.source,
    }
}

fn inventory_matches_library_name(path: &str, name: &str) -> bool {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    if file_name == format!("lib{}.a", name)
        || file_name == format!("lib{}.so", name)
        || file_name == format!("lib{}.dylib", name)
        || file_name == format!("lib{}.tbd", name)
    {
        return true;
    }

    let so_prefix = format!("lib{}.so.", name);
    let dylib_prefix = format!("lib{}.", name);
    file_name.starts_with(&so_prefix)
        || (file_name.starts_with(&dylib_prefix) && file_name.ends_with(".dylib"))
        || (file_name.starts_with(&dylib_prefix) && file_name.ends_with(".tbd"))
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
        assert!(plan.transitive_dependencies.is_empty());
        assert!(plan
            .requirements
            .iter()
            .all(|req| req.source == LinkRequirementSource::Declared));
        assert!(plan
            .requirements
            .iter()
            .all(|req| req.resolution == RequirementResolution::Unresolved));
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
                dependency_edges: vec!["libc.so.6".into()],
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
        assert_eq!(plan.requirements[0].source, LinkRequirementSource::Declared);
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Resolved
        );
        assert_eq!(
            plan.requirements[0].providers[0].match_kind,
            ProviderMatchKind::LibraryName
        );
        assert_eq!(
            plan.requirements[0].providers[0].provenance,
            ProviderProvenance::DiscoveredInventory
        );
        assert_eq!(
            plan.requirements[0].providers[0].dependency_edges,
            vec!["libc.so.6".to_string()]
        );
        assert_eq!(plan.requirements[1].providers.len(), 1);
        assert_eq!(
            plan.requirements[1].resolution,
            RequirementResolution::Resolved
        );
        assert_eq!(
            plan.requirements[1].providers[0].provenance,
            ProviderProvenance::DeclaredArtifact
        );
        assert_eq!(
            plan.requirements[1].providers[0].match_kind,
            ProviderMatchKind::ExactArtifact
        );
        assert_eq!(plan.transitive_dependencies, vec!["libc.so.6".to_string()]);
    }

    #[test]
    fn resolve_link_plan_marks_ambiguous_requirements() {
        let mut package = BindingPackage::new();
        package.link = BindingLinkSurface {
            ordered_inputs: vec![LinkInput::Library(LinkLibrary {
                name: "z".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            })],
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
                artifact_path: "/opt/lib/libz.a".into(),
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
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Ambiguous
        );
        assert_eq!(plan.requirements[0].providers.len(), 2);
    }

    #[test]
    fn resolve_link_plan_for_target_filters_non_matching_constraints() {
        let mut package = BindingPackage::new();
        package.link = BindingLinkSurface {
            platform_constraints: vec!["linux".into()],
            ordered_inputs: vec![LinkInput::Library(LinkLibrary {
                name: "z".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            })],
            ..BindingLinkSurface::default()
        };

        let matching =
            resolve_link_plan_for_target(&package, &[], Some("x86_64-unknown-linux-gnu"));
        assert_eq!(matching.inputs.len(), 1);
        assert_eq!(matching.requirements.len(), 1);
        assert_eq!(matching.platform_constraints, vec!["linux".to_string()]);

        let non_matching = resolve_link_plan_for_target(&package, &[], Some("x86_64-apple-darwin"));
        assert!(non_matching.inputs.is_empty());
        assert!(non_matching.requirements.is_empty());
        assert!(non_matching.platform_constraints.is_empty());
    }

    #[test]
    fn resolve_link_plan_matches_versioned_shared_library_filenames() {
        let mut package = BindingPackage::new();
        package.link = BindingLinkSurface {
            ordered_inputs: vec![LinkInput::Library(LinkLibrary {
                name: "ssl".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            })],
            ..BindingLinkSurface::default()
        };
        let inventories = vec![SymbolInventory {
            artifact_path: "/usr/lib/x86_64-linux-gnu/libssl.so.3".into(),
            format: ArtifactFormat::ElfSharedLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::SharedLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: true,
            },
            dependency_edges: vec!["libcrypto.so.3".into()],
            symbols: Vec::new(),
        }];

        let plan = resolve_link_plan_with_inventories(&package, &inventories);
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Resolved
        );
        assert_eq!(plan.requirements[0].providers.len(), 1);
        assert_eq!(
            plan.requirements[0].providers[0].artifact_path,
            "/usr/lib/x86_64-linux-gnu/libssl.so.3"
        );
        assert_eq!(
            plan.transitive_dependencies,
            vec!["libcrypto.so.3".to_string()]
        );
    }

    #[test]
    fn resolved_link_plan_json_roundtrip() {
        let plan = ResolvedLinkPlan {
            preferred_mode: LinkResolutionMode::PreferStatic,
            native_surface_kind: NativeSurfaceKind::ConcreteArtifacts,
            platform_constraints: vec!["linux".into()],
            inputs: vec![LinkInput::Library(LinkLibrary {
                name: "z".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            })],
            requirements: vec![ResolvedLinkRequirement {
                declared: LinkInput::Library(LinkLibrary {
                    name: "z".into(),
                    kind: LinkLibraryKind::Default,
                    source: LinkRequirementSource::Declared,
                }),
                source: LinkRequirementSource::Declared,
                resolution: RequirementResolution::Resolved,
                providers: vec![ResolvedProvider {
                    artifact_path: "/usr/lib/libz.so".into(),
                    match_kind: ProviderMatchKind::LibraryName,
                    provenance: ProviderProvenance::DiscoveredInventory,
                    dependency_edges: vec!["libc.so.6".into()],
                }],
            }],
            transitive_dependencies: vec!["libc.so.6".into()],
        };

        let json = serde_json::to_string_pretty(&plan).unwrap();
        let plan2: ResolvedLinkPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, plan2);
    }

    #[test]
    fn resolve_link_plan_matches_macos_text_stub_library_names() {
        let mut package = BindingPackage::new();
        package
            .link
            .ordered_inputs
            .push(LinkInput::Library(LinkLibrary {
                name: "System".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            }));

        let inventories = vec![SymbolInventory {
            artifact_path: "/usr/lib/libSystem.tbd".into(),
            format: ArtifactFormat::MachODylib,
            platform: ArtifactPlatform::MachO,
            kind: ArtifactKind::SharedLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: true,
            },
            dependency_edges: vec!["/usr/lib/libc++.1.dylib".into()],
            symbols: Vec::new(),
        }];

        let plan = resolve_link_plan_with_inventories(&package, &inventories);
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Resolved
        );
        assert_eq!(plan.requirements[0].providers.len(), 1);
        assert_eq!(
            plan.requirements[0].providers[0].artifact_path,
            "/usr/lib/libSystem.tbd"
        );
        assert_eq!(
            plan.transitive_dependencies,
            vec!["/usr/lib/libc++.1.dylib".to_string()]
        );
    }

    #[test]
    fn resolve_link_plan_reports_unresolved_when_no_inventory_matches() {
        let mut package = BindingPackage::new();
        package
            .link
            .ordered_inputs
            .push(LinkInput::Library(LinkLibrary {
                name: "missing".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            }));

        let plan = resolve_link_plan_with_inventories(&package, &[]);
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Unresolved
        );
        assert!(plan.requirements[0].providers.is_empty());
    }

    #[test]
    fn resolve_link_plan_reports_ambiguous_when_multiple_inventories_match() {
        let mut package = BindingPackage::new();
        package
            .link
            .ordered_inputs
            .push(LinkInput::Library(LinkLibrary {
                name: "z".into(),
                kind: LinkLibraryKind::Default,
                source: LinkRequirementSource::Declared,
            }));

        let inventories = vec![
            SymbolInventory {
                artifact_path: "/usr/lib/libz.so".into(),
                format: ArtifactFormat::ElfSharedLibrary,
                platform: ArtifactPlatform::Elf,
                kind: ArtifactKind::SharedLibrary,
                capabilities: ArtifactCapabilities {
                    exports_symbols: true,
                    imports_symbols: false,
                },
                dependency_edges: Vec::new(),
                symbols: Vec::new(),
            },
            SymbolInventory {
                artifact_path: "/opt/lib/libz.a".into(),
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
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Ambiguous
        );
        assert_eq!(plan.requirements[0].providers.len(), 2);
    }

    #[test]
    fn resolve_link_plan_concrete_artifact_matches_declared_path() {
        let mut package = BindingPackage::new();
        package
            .link
            .ordered_inputs
            .push(LinkInput::Artifact(LinkArtifact {
                path: "/build/libvendor.a".into(),
                kind: LinkArtifactKind::StaticLibrary,
                source: LinkRequirementSource::Declared,
            }));

        let inventories = vec![SymbolInventory {
            artifact_path: "/build/libvendor.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: Vec::new(),
        }];

        let plan = resolve_link_plan_with_inventories(&package, &inventories);
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(
            plan.requirements[0].resolution,
            RequirementResolution::Resolved
        );
        assert_eq!(
            plan.requirements[0].providers[0].match_kind,
            ProviderMatchKind::ExactArtifact
        );
    }
}
