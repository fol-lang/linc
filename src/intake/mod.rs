//! Frontend-neutral intake layer for LINC.
//!
//! This module defines the source contract that LINC consumes. It is
//! intentionally independent of any specific parser or frontend AST so that
//! another frontend can be substituted without touching LINC core logic.
//!
//! The primary entry type is [`SourcePackage`], a normalized collection of
//! declarations, macros, link requirements, and provenance that a frontend
//! produces after scanning and extracting source-level information.

pub mod adapters;
pub mod source;

pub use source::{
    SourceDeclaration, SourceEnum, SourceEnumVariant, SourceField, SourceFunction, SourceLinkKind,
    SourceLinkRequirement, SourceMacro, SourcePackage, SourceParameter, SourceRecord, SourceType,
    SourceTypeAlias, SourceVariable,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_package() {
        let pkg = SourcePackage::default();
        assert!(pkg.declarations.is_empty());
        assert!(pkg.macros.is_empty());
        assert!(pkg.link_requirements.is_empty());
        assert!(pkg.source_path.is_none());
    }

    #[test]
    fn source_package_with_declarations() {
        let mut pkg = SourcePackage::default();
        pkg.declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "malloc".into(),
                parameters: vec![SourceParameter {
                    name: Some("size".into()),
                    ty: SourceType::ULong,
                }],
                return_type: SourceType::Pointer(Box::new(SourceType::Void)),
                variadic: false,
                source_offset: None,
            }));
        pkg.declarations
            .push(SourceDeclaration::Record(SourceRecord {
                name: Some("point".into()),
                is_union: false,
                fields: Some(vec![
                    SourceField {
                        name: Some("x".into()),
                        ty: SourceType::Int,
                        bit_width: None,
                    },
                    SourceField {
                        name: Some("y".into()),
                        ty: SourceType::Int,
                        bit_width: None,
                    },
                ]),
                source_offset: None,
            }));
        assert_eq!(pkg.declarations.len(), 2);
    }

    #[test]
    fn source_type_pointer_chain() {
        let ty = SourceType::Pointer(Box::new(SourceType::Pointer(Box::new(SourceType::Int))));
        match ty {
            SourceType::Pointer(inner) => match *inner {
                SourceType::Pointer(inner2) => assert_eq!(*inner2, SourceType::Int),
                _ => panic!("expected nested pointer"),
            },
            _ => panic!("expected pointer"),
        }
    }

    #[test]
    fn source_package_json_roundtrip() {
        let mut pkg = SourcePackage {
            source_path: Some("demo.h".into()),
            ..SourcePackage::default()
        };
        pkg.declarations
            .push(SourceDeclaration::Function(SourceFunction {
                name: "foo".into(),
                parameters: vec![],
                return_type: SourceType::Void,
                variadic: false,
                source_offset: Some(10),
            }));
        pkg.macros.push(SourceMacro {
            name: "VERSION".into(),
            body: "3".into(),
            function_like: false,
        });
        let json = serde_json::to_string_pretty(&pkg).unwrap();
        let pkg2: SourcePackage = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg, pkg2);
    }
}
