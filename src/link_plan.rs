use serde::{Deserialize, Serialize};

use crate::ir::{
    BindingPackage, LinkInput, LinkResolutionMode, NativeSurfaceKind,
};

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
}

pub fn resolve_link_plan(package: &BindingPackage) -> ResolvedLinkPlan {
    ResolvedLinkPlan {
        preferred_mode: package.link.preferred_mode,
        native_surface_kind: package.link.native_surface_kind,
        platform_constraints: package.link.platform_constraints.clone(),
        inputs: package.link.ordered_inputs.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BindingLinkSurface, LinkFramework, LinkLibrary, LinkLibraryKind, LinkRequirementSource,
    };

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
    }
}
