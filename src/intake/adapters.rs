//! Adapters for converting between frontend outputs and LINC intake types.
//!
//! The primary adapter converts a [`BindingPackage`] (produced by the current
//! extract/preprocess pipeline) into a [`SourcePackage`] for the intake layer.
//! A reverse adapter converts intake types back to IR types for use by LINC
//! core logic that still operates on the existing IR.

use crate::intake::source::*;
use crate::ir;

/// Convert a [`BindingPackage`] into a [`SourcePackage`].
///
/// This is the primary intake adapter during the migration period. Once
/// frontends produce [`SourcePackage`] directly, this adapter becomes
/// unnecessary.
pub fn from_binding_package(pkg: &ir::BindingPackage) -> SourcePackage {
    let declarations = pkg
        .items
        .iter()
        .filter_map(|item| match item {
            ir::BindingItem::Function(f) => Some(SourceDeclaration::Function(SourceFunction {
                name: f.name.clone(),
                parameters: f
                    .parameters
                    .iter()
                    .map(|p| SourceParameter {
                        name: p.name.clone(),
                        ty: binding_type_to_source(&p.ty),
                    })
                    .collect(),
                return_type: binding_type_to_source(&f.return_type),
                variadic: f.variadic,
                source_offset: f.source_offset,
            })),
            ir::BindingItem::Record(r) => Some(SourceDeclaration::Record(SourceRecord {
                name: r.name.clone(),
                is_union: r.kind == ir::RecordKind::Union,
                fields: r.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|field| SourceField {
                            name: field.name.clone(),
                            ty: binding_type_to_source(&field.ty),
                            bit_width: field.bit_width,
                        })
                        .collect()
                }),
                source_offset: r.source_offset,
            })),
            ir::BindingItem::Enum(e) => Some(SourceDeclaration::Enum(SourceEnum {
                name: e.name.clone(),
                variants: e
                    .variants
                    .iter()
                    .map(|v| SourceEnumVariant {
                        name: v.name.clone(),
                        value: v.value,
                    })
                    .collect(),
                source_offset: e.source_offset,
            })),
            ir::BindingItem::TypeAlias(a) => Some(SourceDeclaration::TypeAlias(SourceTypeAlias {
                name: a.name.clone(),
                target: binding_type_to_source(&a.target),
                source_offset: a.source_offset,
            })),
            ir::BindingItem::Variable(v) => Some(SourceDeclaration::Variable(SourceVariable {
                name: v.name.clone(),
                ty: binding_type_to_source(&v.ty),
                source_offset: v.source_offset,
            })),
            ir::BindingItem::Unsupported(_) => None,
        })
        .collect();

    let macros = pkg
        .macros
        .iter()
        .map(|m| SourceMacro {
            name: m.name.clone(),
            body: m.body.clone(),
            function_like: m.function_like,
        })
        .collect();

    let link_requirements = pkg
        .link
        .libraries
        .iter()
        .map(|lib| SourceLinkRequirement {
            name: lib.name.clone(),
            kind: match lib.kind {
                ir::LinkLibraryKind::Default => SourceLinkKind::Library,
                ir::LinkLibraryKind::Static => SourceLinkKind::StaticLibrary,
                ir::LinkLibraryKind::Dynamic => SourceLinkKind::DynamicLibrary,
            },
        })
        .collect();

    SourcePackage {
        source_path: pkg.source_path.clone(),
        declarations,
        macros,
        link_requirements,
        include_dirs: pkg.inputs.include_dirs.clone(),
        entry_headers: pkg.inputs.entry_headers.clone(),
        defines: pkg
            .inputs
            .defines
            .iter()
            .map(|d| (d.name.clone(), d.value.clone()))
            .collect(),
        target_triple: pkg.target.target_triple.clone(),
        compiler_command: pkg.target.compiler_command.clone(),
        compiler_version: pkg.target.compiler_version.clone(),
    }
}

/// Convert a [`SourcePackage`] into a [`BindingPackage`].
///
/// This is the reverse adapter that allows LINC core logic to operate on
/// its existing IR types while accepting intake-layer input.
pub fn to_binding_package(src: &SourcePackage) -> ir::BindingPackage {
    let items: Vec<ir::BindingItem> = src
        .declarations
        .iter()
        .map(|decl| match decl {
            SourceDeclaration::Function(f) => ir::BindingItem::Function(ir::FunctionBinding {
                name: f.name.clone(),
                calling_convention: ir::CallingConvention::C,
                parameters: f
                    .parameters
                    .iter()
                    .map(|p| ir::ParameterBinding {
                        name: p.name.clone(),
                        ty: source_type_to_binding(&p.ty),
                    })
                    .collect(),
                return_type: source_type_to_binding(&f.return_type),
                variadic: f.variadic,
                source_offset: f.source_offset,
            }),
            SourceDeclaration::Record(r) => ir::BindingItem::Record(ir::RecordBinding {
                kind: if r.is_union {
                    ir::RecordKind::Union
                } else {
                    ir::RecordKind::Struct
                },
                name: r.name.clone(),
                fields: r.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|field| ir::FieldBinding {
                            name: field.name.clone(),
                            ty: source_type_to_binding(&field.ty),
                            bit_width: field.bit_width,
                            layout: None,
                        })
                        .collect()
                }),
                representation: None,
                abi_confidence: None,
                source_offset: r.source_offset,
            }),
            SourceDeclaration::Enum(e) => ir::BindingItem::Enum(ir::EnumBinding {
                name: e.name.clone(),
                variants: e
                    .variants
                    .iter()
                    .map(|v| ir::EnumVariant {
                        name: v.name.clone(),
                        value: v.value,
                    })
                    .collect(),
                representation: None,
                abi_confidence: None,
                source_offset: e.source_offset,
            }),
            SourceDeclaration::TypeAlias(a) => ir::BindingItem::TypeAlias(ir::TypeAliasBinding {
                name: a.name.clone(),
                target: source_type_to_binding(&a.target),
                canonical_resolution: None,
                abi_confidence: None,
                source_offset: a.source_offset,
            }),
            SourceDeclaration::Variable(v) => ir::BindingItem::Variable(ir::VariableBinding {
                name: v.name.clone(),
                ty: source_type_to_binding(&v.ty),
                source_offset: v.source_offset,
            }),
        })
        .collect();

    let macros: Vec<ir::MacroBinding> = src
        .macros
        .iter()
        .map(|m| ir::MacroBinding {
            name: m.name.clone(),
            body: m.body.clone(),
            function_like: m.function_like,
            form: if m.function_like {
                ir::MacroForm::FunctionLike
            } else {
                ir::MacroForm::ObjectLike
            },
            kind: ir::MacroKind::Other,
            category: ir::MacroCategory::default(),
            value: None,
        })
        .collect();

    let libraries: Vec<ir::LinkLibrary> = src
        .link_requirements
        .iter()
        .filter(|r| !matches!(r.kind, SourceLinkKind::Framework))
        .map(|r| ir::LinkLibrary {
            name: r.name.clone(),
            kind: match r.kind {
                SourceLinkKind::Library => ir::LinkLibraryKind::Default,
                SourceLinkKind::StaticLibrary => ir::LinkLibraryKind::Static,
                SourceLinkKind::DynamicLibrary => ir::LinkLibraryKind::Dynamic,
                SourceLinkKind::Framework => ir::LinkLibraryKind::Default,
            },
            source: ir::LinkRequirementSource::Declared,
        })
        .collect();

    ir::BindingPackage {
        source_path: src.source_path.clone(),
        items,
        macros,
        link: ir::BindingLinkSurface {
            libraries,
            ..ir::BindingLinkSurface::default()
        },
        inputs: ir::BindingInputs {
            entry_headers: src.entry_headers.clone(),
            include_dirs: src.include_dirs.clone(),
            defines: src
                .defines
                .iter()
                .map(|(k, v)| ir::BindingDefine {
                    name: k.clone(),
                    value: v.clone(),
                })
                .collect(),
        },
        target: ir::BindingTarget {
            target_triple: src.target_triple.clone(),
            compiler_command: src.compiler_command.clone(),
            compiler_version: src.compiler_version.clone(),
            flavor: None,
        },
        diagnostics: Vec::new(),
        ..ir::BindingPackage::new()
    }
}

fn binding_type_to_source(ty: &ir::BindingType) -> SourceType {
    match ty {
        ir::BindingType::Void => SourceType::Void,
        ir::BindingType::Bool => SourceType::Bool,
        ir::BindingType::Char => SourceType::Char,
        ir::BindingType::SChar => SourceType::SChar,
        ir::BindingType::UChar => SourceType::UChar,
        ir::BindingType::Short => SourceType::Short,
        ir::BindingType::UShort => SourceType::UShort,
        ir::BindingType::Int => SourceType::Int,
        ir::BindingType::UInt => SourceType::UInt,
        ir::BindingType::Long => SourceType::Long,
        ir::BindingType::ULong => SourceType::ULong,
        ir::BindingType::LongLong => SourceType::LongLong,
        ir::BindingType::ULongLong => SourceType::ULongLong,
        ir::BindingType::Float => SourceType::Float,
        ir::BindingType::Double => SourceType::Double,
        ir::BindingType::LongDouble => SourceType::LongDouble,
        ir::BindingType::Pointer {
            pointee,
            const_pointee,
            ..
        } => {
            let inner = binding_type_to_source(pointee);
            if *const_pointee {
                SourceType::ConstPointer(Box::new(inner))
            } else {
                SourceType::Pointer(Box::new(inner))
            }
        }
        ir::BindingType::Array(elem, size) => {
            SourceType::Array(Box::new(binding_type_to_source(elem)), *size)
        }
        ir::BindingType::Qualified { ty, qualifiers } => {
            let inner = binding_type_to_source(ty);
            if qualifiers.is_const {
                SourceType::Const(Box::new(inner))
            } else if qualifiers.is_volatile {
                SourceType::Volatile(Box::new(inner))
            } else {
                inner
            }
        }
        ir::BindingType::FunctionPointer {
            return_type,
            parameters,
            variadic,
        } => SourceType::FunctionPointer {
            return_type: Box::new(binding_type_to_source(return_type)),
            parameters: parameters.iter().map(binding_type_to_source).collect(),
            variadic: *variadic,
        },
        ir::BindingType::TypedefRef(name) => SourceType::TypedefRef(name.clone()),
        ir::BindingType::RecordRef(name) => SourceType::RecordRef(name.clone()),
        ir::BindingType::EnumRef(name) => SourceType::EnumRef(name.clone()),
        ir::BindingType::Opaque(name) => SourceType::Opaque(name.clone()),
    }
}

fn source_type_to_binding(ty: &SourceType) -> ir::BindingType {
    match ty {
        SourceType::Void => ir::BindingType::Void,
        SourceType::Bool => ir::BindingType::Bool,
        SourceType::Char => ir::BindingType::Char,
        SourceType::SChar => ir::BindingType::SChar,
        SourceType::UChar => ir::BindingType::UChar,
        SourceType::Short => ir::BindingType::Short,
        SourceType::UShort => ir::BindingType::UShort,
        SourceType::Int => ir::BindingType::Int,
        SourceType::UInt => ir::BindingType::UInt,
        SourceType::Long => ir::BindingType::Long,
        SourceType::ULong => ir::BindingType::ULong,
        SourceType::LongLong => ir::BindingType::LongLong,
        SourceType::ULongLong => ir::BindingType::ULongLong,
        SourceType::Float => ir::BindingType::Float,
        SourceType::Double => ir::BindingType::Double,
        SourceType::LongDouble => ir::BindingType::LongDouble,
        SourceType::Pointer(inner) => ir::BindingType::ptr(source_type_to_binding(inner)),
        SourceType::ConstPointer(inner) => {
            ir::BindingType::const_ptr(source_type_to_binding(inner))
        }
        SourceType::Array(elem, size) => {
            ir::BindingType::Array(Box::new(source_type_to_binding(elem)), *size)
        }
        SourceType::FunctionPointer {
            return_type,
            parameters,
            variadic,
        } => ir::BindingType::FunctionPointer {
            return_type: Box::new(source_type_to_binding(return_type)),
            parameters: parameters.iter().map(source_type_to_binding).collect(),
            variadic: *variadic,
        },
        SourceType::TypedefRef(name) => ir::BindingType::TypedefRef(name.clone()),
        SourceType::RecordRef(name) => ir::BindingType::RecordRef(name.clone()),
        SourceType::EnumRef(name) => ir::BindingType::EnumRef(name.clone()),
        SourceType::Opaque(name) => ir::BindingType::Opaque(name.clone()),
        SourceType::Const(inner) => ir::BindingType::qualified(
            source_type_to_binding(inner),
            ir::TypeQualifiers {
                is_const: true,
                ..Default::default()
            },
        ),
        SourceType::Volatile(inner) => ir::BindingType::qualified(
            source_type_to_binding(inner),
            ir::TypeQualifiers {
                is_volatile: true,
                ..Default::default()
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir;

    #[test]
    fn roundtrip_binding_to_source_to_binding() {
        let mut pkg = ir::BindingPackage::new();
        pkg.source_path = Some("test.h".into());
        pkg.items
            .push(ir::BindingItem::Function(ir::FunctionBinding {
                name: "foo".into(),
                calling_convention: ir::CallingConvention::C,
                parameters: vec![ir::ParameterBinding {
                    name: Some("x".into()),
                    ty: ir::BindingType::Int,
                }],
                return_type: ir::BindingType::Void,
                variadic: false,
                source_offset: Some(10),
            }));
        pkg.items.push(ir::BindingItem::Record(ir::RecordBinding {
            kind: ir::RecordKind::Struct,
            name: Some("point".into()),
            fields: Some(vec![ir::FieldBinding {
                name: Some("x".into()),
                ty: ir::BindingType::Int,
                bit_width: None,
                layout: None,
            }]),
            representation: None,
            abi_confidence: None,
            source_offset: Some(20),
        }));
        pkg.items.push(ir::BindingItem::Enum(ir::EnumBinding {
            name: Some("color".into()),
            variants: vec![ir::EnumVariant {
                name: "RED".into(),
                value: Some(0),
            }],
            representation: None,
            abi_confidence: None,
            source_offset: Some(30),
        }));
        pkg.items
            .push(ir::BindingItem::TypeAlias(ir::TypeAliasBinding {
                name: "myint".into(),
                target: ir::BindingType::Int,
                canonical_resolution: None,
                abi_confidence: None,
                source_offset: Some(40),
            }));
        pkg.items
            .push(ir::BindingItem::Variable(ir::VariableBinding {
                name: "errno".into(),
                ty: ir::BindingType::Int,
                source_offset: Some(50),
            }));

        let source = from_binding_package(&pkg);
        assert_eq!(source.declarations.len(), 5);
        assert_eq!(source.source_path.as_deref(), Some("test.h"));

        let back = to_binding_package(&source);
        assert_eq!(back.items.len(), 5);
        assert_eq!(back.source_path.as_deref(), Some("test.h"));

        // Verify each item type survived
        assert_eq!(back.function_count(), 1);
        assert_eq!(back.record_count(), 1);
        assert_eq!(back.enum_count(), 1);
        assert_eq!(back.type_alias_count(), 1);
        assert_eq!(back.variable_count(), 1);
    }

    #[test]
    fn adapter_preserves_macros_and_link_requirements() {
        let mut pkg = ir::BindingPackage::new();
        pkg.macros.push(ir::MacroBinding {
            name: "VERSION".into(),
            body: "3".into(),
            function_like: false,
            form: ir::MacroForm::ObjectLike,
            kind: ir::MacroKind::Integer,
            category: ir::MacroCategory::BindableConstant,
            value: Some(ir::MacroValue::Integer(3)),
        });
        pkg.link.libraries.push(ir::LinkLibrary {
            name: "z".into(),
            kind: ir::LinkLibraryKind::Dynamic,
            source: ir::LinkRequirementSource::Declared,
        });

        let source = from_binding_package(&pkg);
        assert_eq!(source.macros.len(), 1);
        assert_eq!(source.macros[0].name, "VERSION");
        assert_eq!(source.link_requirements.len(), 1);
        assert_eq!(source.link_requirements[0].name, "z");
        assert_eq!(
            source.link_requirements[0].kind,
            SourceLinkKind::DynamicLibrary
        );
    }

    #[test]
    fn adapter_skips_unsupported_items() {
        let mut pkg = ir::BindingPackage::new();
        pkg.items
            .push(ir::BindingItem::Unsupported(ir::UnsupportedItem {
                name: Some("flags".into()),
                reason: "bitfield".into(),
                source_offset: None,
            }));
        pkg.items
            .push(ir::BindingItem::Function(ir::FunctionBinding {
                name: "foo".into(),
                calling_convention: ir::CallingConvention::C,
                parameters: vec![],
                return_type: ir::BindingType::Void,
                variadic: false,
                source_offset: None,
            }));

        let source = from_binding_package(&pkg);
        assert_eq!(source.declarations.len(), 1);
    }

    #[test]
    fn type_conversion_complex_types() {
        let ty = ir::BindingType::FunctionPointer {
            return_type: Box::new(ir::BindingType::Int),
            parameters: vec![
                ir::BindingType::const_ptr(ir::BindingType::Char),
                ir::BindingType::ptr(ir::BindingType::Void),
            ],
            variadic: true,
        };
        let source_ty = binding_type_to_source(&ty);
        let back = source_type_to_binding(&source_ty);

        match back {
            ir::BindingType::FunctionPointer {
                return_type,
                parameters,
                variadic,
            } => {
                assert_eq!(*return_type, ir::BindingType::Int);
                assert_eq!(parameters.len(), 2);
                assert!(variadic);
            }
            _ => panic!("expected FunctionPointer"),
        }
    }
}
