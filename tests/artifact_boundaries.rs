mod common;

use linc::{analyze_source_package, SourceDeclaration, SourcePackage};
use linc::ir::LinkInput;

fn parc_source_artifact(source: &str) -> parc::ir::SourcePackage {
    let package = parc::extract::extract_from_source(source).expect("parc extraction should work");
    let json = serde_json::to_string_pretty(&package).expect("parc artifact json");
    serde_json::from_str(&json).expect("parc artifact roundtrip")
}

#[test]
fn parc_artifact_roundtrip_can_drive_linc_analysis() {
    let parc_pkg = parc_source_artifact(
        r#"
        typedef unsigned long size_t;
        struct point { int x; int y; };
        extern int demo_init(struct point* p, size_t count);
        "#,
    );

    let binding = common::from_parc_package(&parc_pkg);
    let mut source: SourcePackage = linc::intake::adapters::from_binding_package(&binding);
    source.link_requirements.push(linc::SourceLinkRequirement {
        name: "demo".into(),
        kind: linc::SourceLinkKind::DynamicLibrary,
    });

    let analysis = analyze_source_package(&source);

    assert_eq!(source.declarations.len(), 3);
    assert!(matches!(
        source.declarations[0],
        SourceDeclaration::TypeAlias(_)
    ));
    assert!(source
        .declarations
        .iter()
        .any(|decl| matches!(decl, SourceDeclaration::Record(record) if record.name.as_deref() == Some("point"))));
    assert!(source
        .declarations
        .iter()
        .any(|decl| matches!(decl, SourceDeclaration::Function(function) if function.name == "demo_init")));

    let resolved = analysis
        .resolved_link_plan
        .as_ref()
        .expect("analysis should resolve a planning surface");
    assert_eq!(resolved.inputs.len(), 1);
    assert!(matches!(
        &resolved.inputs[0],
        LinkInput::Library(library) if library.name == "demo"
    ));
    assert!(analysis
        .declared_link_surface
        .libraries
        .iter()
        .any(|library| library.name == "demo"));
}
