mod common;
use linc::{
    from_source_package, resolve_link_plan_for_target, validate, BindingPackage, LinkInput,
    LinkLibrary, LinkLibraryKind, LinkRequirementSource, MatchStatus, SourceDeclaration,
    SourceFunction, SourcePackage, SourceType, SymbolInventory,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FolBindingInput {
    schema_version: u32,
    items: Vec<Value>,
    layouts: Vec<FolLayout>,
    diagnostics: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct FolLayout {
    name: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct FolAbiSensitiveBindingInput {
    schema_version: u32,
    items: Vec<Value>,
    layouts: Vec<FolLayout>,
}

#[derive(Debug, Deserialize)]
struct FolNativeBundle {
    package: FolNativePackage,
    validation: FolValidationReport,
    link_plan: FolResolvedLinkPlan,
}

#[derive(Debug, Deserialize)]
struct FolNativePackage {
    schema_version: u32,
    link: FolPackageLinkSurface,
}

#[derive(Debug, Deserialize)]
struct FolPackageLinkSurface {
    platform_constraints: Vec<String>,
    ordered_inputs: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct FolValidationReport {
    summary: FolValidationSummary,
    matches: Vec<FolValidationMatch>,
}

#[derive(Debug, Deserialize)]
struct FolValidationSummary {
    #[serde(default)]
    matched: usize,
    #[serde(default)]
    missing: usize,
    #[serde(default)]
    abi_shape_mismatches: usize,
    #[serde(default)]
    duplicate_providers: usize,
}

#[derive(Debug, Deserialize)]
struct FolValidationMatch {
    name: String,
    status: MatchStatus,
    provider_artifacts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FolValidationGateReport {
    summary: FolValidationSummary,
    matches: Vec<FolValidationMatch>,
}

#[derive(Debug, Deserialize)]
struct FolResolvedLinkPlan {
    platform_constraints: Vec<String>,
    requirements: Vec<FolResolvedRequirement>,
    transitive_dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FolResolvedRequirement {
    resolution: linc::RequirementResolution,
    providers: Vec<FolResolvedProvider>,
}

#[derive(Debug, Deserialize)]
struct FolResolvedProvider {
    artifact_path: String,
}

fn fol_should_gate_on_validation(report: &FolValidationGateReport) -> bool {
    report.summary.abi_shape_mismatches > 0
        || report.summary.missing > 0
        || report.matches.iter().any(|m| {
            matches!(
                m.status,
                MatchStatus::UnresolvedDeclaredLinkInputs | MatchStatus::DuplicateProviders
            )
        })
}

fn fol_link_plan_is_ready(plan: &FolResolvedLinkPlan) -> bool {
    !plan.requirements.is_empty()
        && plan.requirements.iter().all(|requirement| {
            requirement.resolution == linc::RequirementResolution::Resolved
                && !requirement.providers.is_empty()
                && requirement
                    .providers
                    .iter()
                    .all(|provider| !provider.artifact_path.is_empty())
        })
}

#[test]
fn fol_acceptance_binding_scan_flow_stays_consumable() {
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tricky_layouts.h");
    let result = common::process(&linc::HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("struct packed_flags")
        .probe_type_layout("enum widget_mode"))
        .unwrap();

    let json = serde_json::to_string(&result.package).unwrap();
    let consumed: FolBindingInput = serde_json::from_str(&json).unwrap();

    assert_eq!(consumed.schema_version, linc::SCHEMA_VERSION);
    assert!(consumed
        .diagnostics
        .iter()
        .all(|diag| diag.get("severity").and_then(Value::as_str).is_some()));
    assert!(consumed
        .items
        .iter()
        .any(|item| item.get("Record").is_some()));
    assert!(consumed.items.iter().any(|item| item.get("Enum").is_some()));
    assert!(consumed
        .items
        .iter()
        .any(|item| item.get("TypeAlias").is_some()));
    assert!(consumed
        .layouts
        .iter()
        .any(|layout| layout.name == "struct packed_flags" && layout.size > 0));
    assert!(consumed
        .layouts
        .iter()
        .any(|layout| layout.name == "enum widget_mode" && layout.size > 0));
}

#[test]
fn fol_acceptance_layout_backed_binding_flow_stays_consumable() {
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typedef_layout_bridge.h");
    let result = common::process(&linc::HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("widget_t")
        .probe_type_layout("mode_t"))
        .unwrap();

    let json = serde_json::to_string(&result.package).unwrap();
    let consumed: FolAbiSensitiveBindingInput = serde_json::from_str(&json).unwrap();

    assert_eq!(consumed.schema_version, linc::SCHEMA_VERSION);
    assert!(consumed
        .items
        .iter()
        .any(|item| item.get("TypeAlias").is_some()));
    assert!(consumed
        .items
        .iter()
        .any(|item| item.get("Variable").is_some()));
    assert!(consumed
        .layouts
        .iter()
        .any(|layout| layout.name == "widget_t" && layout.size > 0));
    assert!(consumed
        .layouts
        .iter()
        .any(|layout| layout.name == "mode_t" && layout.size > 0));
}

#[test]
fn fol_acceptance_native_binding_and_link_flow_stays_consumable() {
    let mut package: BindingPackage = from_source_package(&SourcePackage {
        declarations: vec![SourceDeclaration::Function(SourceFunction {
            name: "demo_init".into(),
            parameters: vec![],
            return_type: SourceType::Int,
            variadic: false,
            source_offset: None,
        })],
        ..SourcePackage::default()
    });
    package.link.platform_constraints.push("linux".into());
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));
    package.link.libraries.push(LinkLibrary {
        name: "demo".into(),
        kind: LinkLibraryKind::Default,
        source: LinkRequirementSource::Declared,
    });

    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/linux_elf_inventory_fixture.json"
    ))
    .unwrap();

    let validation = validate(&package, &inventory);
    let link_plan = resolve_link_plan_for_target(
        &package,
        std::slice::from_ref(&inventory),
        Some("x86_64-unknown-linux-gnu"),
    );

    let bundle_json = serde_json::to_string(&json!({
        "package": package,
        "validation": validation,
        "link_plan": link_plan,
    }))
    .unwrap();
    let consumed: FolNativeBundle = serde_json::from_str(&bundle_json).unwrap();

    assert_eq!(consumed.package.schema_version, linc::SCHEMA_VERSION);
    assert_eq!(consumed.package.link.platform_constraints, vec!["linux"]);
    assert_eq!(consumed.package.link.ordered_inputs.len(), 1);
    assert_eq!(consumed.validation.summary.matched, 1);
    assert_eq!(consumed.validation.summary.missing, 0);
    assert_eq!(consumed.validation.summary.abi_shape_mismatches, 0);
    assert_eq!(consumed.validation.matches.len(), 1);
    assert_eq!(consumed.validation.matches[0].name, "demo_init");
    assert_eq!(consumed.validation.matches[0].status, MatchStatus::Matched);
    assert_eq!(
        consumed.validation.matches[0].provider_artifacts,
        vec!["/usr/lib/libdemo.so"]
    );
    assert_eq!(consumed.link_plan.platform_constraints, vec!["linux"]);
    assert_eq!(consumed.link_plan.requirements.len(), 1);
    assert_eq!(
        consumed.link_plan.requirements[0].resolution,
        linc::RequirementResolution::Resolved
    );
    assert_eq!(
        consumed.link_plan.requirements[0].providers[0].artifact_path,
        "/usr/lib/libdemo.so"
    );
    assert_eq!(
        consumed.link_plan.transitive_dependencies,
        vec!["libc.so.6"]
    );
}

#[test]
fn fol_acceptance_validation_findings_gate_generation() {
    let abi_questionable: FolValidationGateReport = serde_json::from_str(include_str!(
        "../tests/contracts/function_abi_questionable_report.json"
    ))
    .unwrap();
    assert!(fol_should_gate_on_validation(&abi_questionable));
    assert_eq!(abi_questionable.summary.abi_shape_mismatches, 1);
    assert_eq!(abi_questionable.matches[0].name, "widget_init");
    assert_eq!(
        abi_questionable.matches[0].status,
        MatchStatus::AbiShapeMismatch
    );

    let duplicate_providers: FolValidationGateReport = serde_json::from_str(include_str!(
        "../tests/contracts/validation_duplicate_provider_report.json"
    ))
    .unwrap();
    assert!(fol_should_gate_on_validation(&duplicate_providers));
    assert_eq!(duplicate_providers.summary.duplicate_providers, 1);
    assert_eq!(
        duplicate_providers.matches[0].status,
        MatchStatus::DuplicateProviders
    );
}

#[test]
fn fol_acceptance_resolved_link_plan_stays_consumable() {
    let mut package: BindingPackage = from_source_package(&SourcePackage {
        declarations: vec![SourceDeclaration::Function(SourceFunction {
            name: "demo_init".into(),
            parameters: vec![],
            return_type: SourceType::Int,
            variadic: false,
            source_offset: None,
        })],
        ..SourcePackage::default()
    });
    package.link.platform_constraints.push("linux".into());
    package
        .link
        .ordered_inputs
        .push(LinkInput::Library(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));
    package.link.libraries.push(LinkLibrary {
        name: "demo".into(),
        kind: LinkLibraryKind::Default,
        source: LinkRequirementSource::Declared,
    });

    let inventory: SymbolInventory = serde_json::from_str(include_str!(
        "../tests/contracts/linux_elf_inventory_fixture.json"
    ))
    .unwrap();

    let plan_json = serde_json::to_string(&resolve_link_plan_for_target(
        &package,
        std::slice::from_ref(&inventory),
        Some("x86_64-unknown-linux-gnu"),
    ))
    .unwrap();
    let consumed: FolResolvedLinkPlan = serde_json::from_str(&plan_json).unwrap();

    assert!(fol_link_plan_is_ready(&consumed));
    assert_eq!(consumed.platform_constraints, vec!["linux"]);
    assert_eq!(consumed.requirements.len(), 1);
    assert_eq!(
        consumed.requirements[0].resolution,
        linc::RequirementResolution::Resolved
    );
    assert_eq!(
        consumed.requirements[0].providers[0].artifact_path,
        "/usr/lib/libdemo.so"
    );
    assert_eq!(consumed.transitive_dependencies, vec!["libc.so.6"]);
}
