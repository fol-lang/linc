mod common;
#[allow(dead_code)]
#[path = "linus/epoll.rs"]
mod epoll;

use std::path::{Path, PathBuf};

use linc::{analyze_source_package, SourceDeclaration, SourcePackage};
use linc::ir::LinkInput;

fn parc_source_artifact(source: &str) -> parc::ir::SourcePackage {
    let package = parc::extract::extract_from_source(source).expect("parc extraction should work");
    let json = serde_json::to_string_pretty(&package).expect("parc artifact json");
    serde_json::from_str(&json).expect("parc artifact roundtrip")
}

fn parc_file_artifact(entry: &Path, include_dirs: &[PathBuf]) -> parc::ir::SourcePackage {
    let mut cpp_options = vec!["-E".to_string()];
    for dir in include_dirs {
        cpp_options.push(format!("-I{}", dir.display()));
    }

    let config = parc::driver::Config {
        cpp_command: std::env::var("CC").unwrap_or_else(|_| "gcc".into()),
        cpp_options,
        flavor: parc::driver::Flavor::GnuC11,
    };

    let parsed = parc::driver::parse(&config, entry).expect("parc driver parse should work");
    let package = parc::extract::extract_from_translation_unit(&parsed.unit, Some(entry.display().to_string()));
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

#[test]
fn ugly_system_header_artifact_roundtrip_stays_consumable() {
    let environment = epoll::epoll_environment().expect("epoll fixture should exist");
    let parc_pkg = parc_file_artifact(&environment.header, &environment.include_dirs);

    let binding = common::from_parc_package(&parc_pkg);
    let source: SourcePackage = linc::intake::adapters::from_binding_package(&binding);
    let analysis = analyze_source_package(&source);

    assert!(source
        .declarations
        .iter()
        .any(|decl| matches!(decl, SourceDeclaration::TypeAlias(alias) if alias.name == "epoll_data_t")));
    assert!(source
        .declarations
        .iter()
        .any(|decl| matches!(decl, SourceDeclaration::Record(record) if record.name.as_deref() == Some("epoll_event"))));
    assert!(analysis.resolved_link_plan.is_some());
}

#[test]
fn vendored_external_library_artifact_roundtrip_stays_consumable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/full_apps/external/libpng/header");
    let include_dir = root.join("include");
    let entry = root.join("main.c");
    let parc_pkg = parc_file_artifact(&entry, &[include_dir]);

    let binding = common::from_parc_package(&parc_pkg);
    let source: SourcePackage = linc::intake::adapters::from_binding_package(&binding);

    assert!(source.declarations.len() >= 10);
    assert!(source
        .declarations
        .iter()
        .any(|decl| matches!(decl, SourceDeclaration::Function(function) if function.name.starts_with("png_"))));
    assert!(source
        .macros
        .iter()
        .any(|mac| mac.name.starts_with("PNG_"))
        || source
            .declarations
            .iter()
            .any(|decl| matches!(decl, SourceDeclaration::TypeAlias(alias) if alias.name.starts_with("png_"))));
}

#[test]
fn failure_fixture_artifact_roundtrip_preserves_bitfield_signal() {
    let header =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packed_bitfield_extreme.h");
    let parc_pkg = parc_file_artifact(&header, &[]);

    let binding = common::from_parc_package(&parc_pkg);
    let source: SourcePackage = linc::intake::adapters::from_binding_package(&binding);

    assert!(source.declarations.iter().any(|decl| {
        matches!(decl, SourceDeclaration::Record(record)
            if record.name.as_deref() == Some("packed_registers")
                && record.fields.as_ref().is_some_and(|fields| fields.iter().any(|field| field.bit_width.is_some())))
    }));
    assert!(source.declarations.iter().any(|decl| {
        matches!(decl, SourceDeclaration::TypeAlias(alias) if alias.name == "packed_registers_t")
    }));
}
