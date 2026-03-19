use pac::ast::*;
use pac::span::Node;

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::ir::*;

pub struct Extractor {
    pub items: Vec<BindingItem>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Extractor {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn extract(mut self, unit: &TranslationUnit) -> (Vec<BindingItem>, Vec<Diagnostic>) {
        for ext_decl in &unit.0 {
            self.extract_external_declaration(ext_decl);
        }
        (self.items, self.diagnostics)
    }

    fn extract_external_declaration(&mut self, ext_decl: &Node<ExternalDeclaration>) {
        match &ext_decl.node {
            ExternalDeclaration::Declaration(decl) => {
                self.extract_declaration(&decl.node, decl.span.start);
            }
            ExternalDeclaration::FunctionDefinition(fdef) => {
                self.extract_function_definition(&fdef.node, fdef.span.start);
            }
            ExternalDeclaration::StaticAssert(_) => {}
        }
    }

    fn extract_declaration(&mut self, decl: &Declaration, offset: usize) {
        let is_typedef = decl.specifiers.iter().any(|s| {
            matches!(
                s.node,
                DeclarationSpecifier::StorageClass(ref sc)
                    if sc.node == StorageClassSpecifier::Typedef
            )
        });

        let is_extern = decl.specifiers.iter().any(|s| {
            matches!(
                s.node,
                DeclarationSpecifier::StorageClass(ref sc)
                    if sc.node == StorageClassSpecifier::Extern
            )
        });

        // Extract standalone struct/union/enum definitions (no declarators)
        if decl.declarators.is_empty() {
            self.extract_standalone_type_specifiers(&decl.specifiers, offset);
            return;
        }

        for init_decl in &decl.declarators {
            let declarator = &init_decl.node.declarator;

            if is_typedef {
                self.extract_typedef(&decl.specifiers, &declarator.node, offset);
            } else if self.is_function_declarator(&declarator.node) {
                self.extract_function_declaration(
                    &decl.specifiers,
                    &declarator.node,
                    is_extern,
                    offset,
                );
            } else {
                self.extract_variable_or_type(
                    &decl.specifiers,
                    &declarator.node,
                    is_extern,
                    offset,
                );
            }
        }
    }

    fn extract_function_definition(&mut self, fdef: &FunctionDefinition, offset: usize) {
        // Function definitions have bodies — we extract signature only
        let name = self.declarator_name(&fdef.declarator.node);
        let Some(name) = name else { return };

        let is_static = fdef.specifiers.iter().any(|s| {
            matches!(
                s.node,
                DeclarationSpecifier::StorageClass(ref sc)
                    if sc.node == StorageClassSpecifier::Static
            )
        });
        if is_static {
            self.diagnostics.push(
                Diagnostic::warning(
                    DiagnosticKind::DeclarationUnsupported,
                    format!("static function '{}' not bindable", name),
                )
                .with_item(&name)
                .with_location(None, offset),
            );
            return;
        }

        self.emit_extension_diagnostics(&fdef.declarator.node.extensions, &name, offset);
        self.emit_specifier_diagnostics(&fdef.specifiers, &name, offset);
        self.emit_derived_diagnostics(&fdef.declarator.node, &name, offset);

        let base_ty = self.resolve_base_type(&fdef.specifiers);
        let base_is_const = self.has_const_qualifier(&fdef.specifiers);
        let (mut return_type, params, variadic) =
            self.resolve_function_parts(&fdef.declarator.node, base_ty);
        if base_is_const {
            return_type = self.mark_innermost_pointer_const(return_type);
        }

        self.items.push(BindingItem::Function(FunctionBinding {
            name,
            calling_convention: CallingConvention::C,
            parameters: params,
            return_type,
            variadic,
            source_offset: Some(offset),
        }));
    }

    fn extract_standalone_type_specifiers(
        &mut self,
        specifiers: &[Node<DeclarationSpecifier>],
        offset: usize,
    ) {
        for spec in specifiers {
            match &spec.node {
                DeclarationSpecifier::TypeSpecifier(ts) => match &ts.node {
                    TypeSpecifier::Struct(st) => {
                        self.extract_record(&st.node, offset);
                    }
                    TypeSpecifier::Enum(et) => {
                        self.extract_enum(&et.node, offset);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn extract_typedef(
        &mut self,
        specifiers: &[Node<DeclarationSpecifier>],
        declarator: &Declarator,
        offset: usize,
    ) {
        let name = match self.declarator_name(declarator) {
            Some(n) => n,
            None => return,
        };

        // Also extract any inline struct/enum definitions from the specifiers
        self.extract_inline_type_definitions(specifiers, offset);

        let target = self.resolve_full_type(specifiers, declarator);
        self.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name,
            target,
            source_offset: Some(offset),
        }));
    }

    fn extract_function_declaration(
        &mut self,
        specifiers: &[Node<DeclarationSpecifier>],
        declarator: &Declarator,
        _is_extern: bool,
        offset: usize,
    ) {
        let name = match self.declarator_name(declarator) {
            Some(n) => n,
            None => return,
        };

        self.emit_extension_diagnostics(&declarator.extensions, &name, offset);
        self.emit_specifier_diagnostics(specifiers, &name, offset);
        self.emit_derived_diagnostics(declarator, &name, offset);

        let base_ty = self.resolve_base_type(specifiers);
        let base_is_const = self.has_const_qualifier(specifiers);
        let (mut return_type, params, variadic) =
            self.resolve_function_parts(declarator, base_ty);
        if base_is_const {
            return_type = self.mark_innermost_pointer_const(return_type);
        }

        self.items.push(BindingItem::Function(FunctionBinding {
            name,
            calling_convention: CallingConvention::C,
            parameters: params,
            return_type,
            variadic,
            source_offset: Some(offset),
        }));
    }

    fn extract_variable_or_type(
        &mut self,
        specifiers: &[Node<DeclarationSpecifier>],
        declarator: &Declarator,
        is_extern: bool,
        offset: usize,
    ) {
        // Extract any inline struct/enum definitions
        self.extract_inline_type_definitions(specifiers, offset);

        if is_extern {
            let name = match self.declarator_name(declarator) {
                Some(n) => n,
                None => return,
            };
            let ty = self.resolve_full_type(specifiers, declarator);
            self.items.push(BindingItem::Variable(VariableBinding {
                name,
                ty,
                source_offset: Some(offset),
            }));
        }
    }

    fn extract_inline_type_definitions(
        &mut self,
        specifiers: &[Node<DeclarationSpecifier>],
        offset: usize,
    ) {
        for spec in specifiers {
            if let DeclarationSpecifier::TypeSpecifier(ts) = &spec.node {
                match &ts.node {
                    TypeSpecifier::Struct(st) if st.node.declarations.is_some() => {
                        self.extract_record(&st.node, offset);
                    }
                    TypeSpecifier::Enum(et) if !et.node.enumerators.is_empty() => {
                        self.extract_enum(&et.node, offset);
                    }
                    _ => {}
                }
            }
        }
    }

    fn extract_record(&mut self, st: &StructType, offset: usize) {
        let kind = match st.kind.node {
            StructKind::Struct => RecordKind::Struct,
            StructKind::Union => RecordKind::Union,
        };
        let name = st.identifier.as_ref().map(|id| id.node.name.clone());

        let fields = st.declarations.as_ref().map(|decls| {
            let mut fields = Vec::new();
            for decl in decls {
                match &decl.node {
                    StructDeclaration::Field(field) => {
                        self.extract_struct_fields(&field.node, &mut fields);
                    }
                    StructDeclaration::StaticAssert(_) => {}
                }
            }
            fields
        });

        self.items.push(BindingItem::Record(RecordBinding {
            kind,
            name,
            fields,
            source_offset: Some(offset),
        }));
    }

    fn extract_struct_fields(&mut self, field: &StructField, out: &mut Vec<FieldBinding>) {
        let base_type_specs: Vec<_> = field
            .specifiers
            .iter()
            .filter_map(|sq| match &sq.node {
                SpecifierQualifier::TypeSpecifier(ts) => Some(ts),
                _ => None,
            })
            .collect();

        if field.declarators.is_empty() {
            // Anonymous field (e.g. anonymous struct/union)
            let ty = self.resolve_base_type_from_type_specs(&base_type_specs);
            out.push(FieldBinding { name: None, ty });
            return;
        }

        let base_is_const = self.has_const_qualifier_sq(&field.specifiers);

        for sd in &field.declarators {
            let name = sd
                .node
                .declarator
                .as_ref()
                .and_then(|d| self.declarator_name(&d.node));

            if sd.node.bit_width.is_some() {
                let field_name = name.as_deref().unwrap_or("<anonymous>");
                self.diagnostics.push(
                    Diagnostic::warning(
                        DiagnosticKind::DeclarationPartial,
                        format!("bitfield width ignored on field '{}'", field_name),
                    )
                    .with_item(field_name),
                );
            }

            let mut ty = match &sd.node.declarator {
                Some(d) => self.apply_derived_type(
                    self.resolve_base_type_from_type_specs(&base_type_specs),
                    &d.node,
                ),
                None => self.resolve_base_type_from_type_specs(&base_type_specs),
            };
            if base_is_const {
                ty = self.mark_innermost_pointer_const(ty);
            }
            out.push(FieldBinding { name, ty });
        }
    }

    fn extract_enum(&mut self, et: &EnumType, offset: usize) {
        let name = et.identifier.as_ref().map(|id| id.node.name.clone());
        let variants: Vec<EnumVariant> = et
            .enumerators
            .iter()
            .map(|e| EnumVariant {
                name: e.node.identifier.node.name.clone(),
                value: e.node.expression.as_ref().and_then(|expr| eval_const_expr(&expr.node)),
            })
            .collect();

        self.items.push(BindingItem::Enum(EnumBinding {
            name,
            variants,
            source_offset: Some(offset),
        }));
    }

    // --- Type resolution helpers ---

    fn resolve_base_type(&self, specifiers: &[Node<DeclarationSpecifier>]) -> BindingType {
        let type_specs: Vec<_> = specifiers
            .iter()
            .filter_map(|s| match &s.node {
                DeclarationSpecifier::TypeSpecifier(ts) => Some(ts),
                _ => None,
            })
            .collect();
        self.resolve_base_type_from_type_specs(&type_specs)
    }

    fn resolve_base_type_from_type_specs(
        &self,
        type_specs: &[&Node<TypeSpecifier>],
    ) -> BindingType {
        if type_specs.is_empty() {
            return BindingType::Int; // C default
        }

        // Collect primitive specifier keywords
        let mut has_void = false;
        let mut has_char = false;
        let mut has_short = false;
        let mut has_int = false;
        let mut long_count = 0u8;
        let mut has_float = false;
        let mut has_double = false;
        let mut has_signed = false;
        let mut has_unsigned = false;
        let mut has_bool = false;

        for ts in type_specs {
            match &ts.node {
                TypeSpecifier::Void => has_void = true,
                TypeSpecifier::Char => has_char = true,
                TypeSpecifier::Short => has_short = true,
                TypeSpecifier::Int => has_int = true,
                TypeSpecifier::Long => long_count += 1,
                TypeSpecifier::Float => has_float = true,
                TypeSpecifier::Double => has_double = true,
                TypeSpecifier::Signed => has_signed = true,
                TypeSpecifier::Unsigned => has_unsigned = true,
                TypeSpecifier::Bool => has_bool = true,
                TypeSpecifier::Struct(st) => {
                    let name = st
                        .node
                        .identifier
                        .as_ref()
                        .map(|id| id.node.name.clone())
                        .unwrap_or_else(|| "<anonymous>".into());
                    return BindingType::RecordRef(name);
                }
                TypeSpecifier::Enum(et) => {
                    let name = et
                        .node
                        .identifier
                        .as_ref()
                        .map(|id| id.node.name.clone())
                        .unwrap_or_else(|| "<anonymous>".into());
                    return BindingType::EnumRef(name);
                }
                TypeSpecifier::TypedefName(id) => {
                    return BindingType::TypedefRef(id.node.name.clone());
                }
                TypeSpecifier::Complex => {
                    return BindingType::Opaque("_Complex".into());
                }
                TypeSpecifier::TypeOf(_) => {
                    return BindingType::Opaque("typeof".into());
                }
                _ => {}
            }
        }

        if has_void {
            return BindingType::Void;
        }
        if has_bool {
            return BindingType::Bool;
        }
        if has_float {
            return BindingType::Float;
        }
        if has_double && long_count > 0 {
            return BindingType::LongDouble;
        }
        if has_double {
            return BindingType::Double;
        }
        if has_char {
            return if has_unsigned {
                BindingType::UChar
            } else if has_signed {
                BindingType::SChar
            } else {
                BindingType::Char
            };
        }
        if has_short {
            return if has_unsigned {
                BindingType::UShort
            } else {
                BindingType::Short
            };
        }
        if long_count >= 2 {
            return if has_unsigned {
                BindingType::ULongLong
            } else {
                BindingType::LongLong
            };
        }
        if long_count == 1 {
            return if has_unsigned {
                BindingType::ULong
            } else {
                BindingType::Long
            };
        }
        if has_unsigned {
            return BindingType::UInt;
        }
        if has_signed || has_int {
            return BindingType::Int;
        }

        BindingType::Int
    }

    fn resolve_full_type(
        &self,
        specifiers: &[Node<DeclarationSpecifier>],
        declarator: &Declarator,
    ) -> BindingType {
        let base = self.resolve_base_type(specifiers);
        let base_is_const = self.has_const_qualifier(specifiers);
        let mut ty = self.apply_derived_type(base, declarator);

        // If the base type had const and the outermost type is a pointer,
        // mark the innermost pointer's pointee as const.
        // In `const char *p`: base=Char(const), derived=[Pointer]
        // The result should be Pointer { pointee: Char, const_pointee: true }
        if base_is_const {
            ty = self.mark_innermost_pointer_const(ty);
        }

        ty
    }

    fn has_const_qualifier(&self, specifiers: &[Node<DeclarationSpecifier>]) -> bool {
        specifiers.iter().any(|s| {
            matches!(
                s.node,
                DeclarationSpecifier::TypeQualifier(ref tq) if tq.node == TypeQualifier::Const
            )
        })
    }

    fn has_const_qualifier_sq(&self, specifiers: &[Node<SpecifierQualifier>]) -> bool {
        specifiers.iter().any(|s| {
            matches!(
                s.node,
                SpecifierQualifier::TypeQualifier(ref tq) if tq.node == TypeQualifier::Const
            )
        })
    }

    /// Mark the innermost pointer in a type as const_pointee.
    /// For `Pointer { pointee: Char, const_pointee: false }` → sets const_pointee=true
    /// For `Pointer { pointee: Pointer { pointee: Char }, ... }` → recurses to inner
    fn mark_innermost_pointer_const(&self, ty: BindingType) -> BindingType {
        match ty {
            BindingType::Pointer {
                pointee,
                const_pointee: _,
            } => {
                // Check if the pointee is also a pointer — if so, recurse
                match *pointee {
                    inner @ BindingType::Pointer { .. } => BindingType::Pointer {
                        pointee: Box::new(self.mark_innermost_pointer_const(inner)),
                        const_pointee: false,
                    },
                    other => BindingType::Pointer {
                        pointee: Box::new(other),
                        const_pointee: true,
                    },
                }
            }
            other => other, // Not a pointer, nothing to mark
        }
    }

    fn apply_derived_type(&self, base: BindingType, declarator: &Declarator) -> BindingType {
        let mut ty = base;

        // Process derived declarators in order
        // In C declarator syntax, the derived list goes from innermost to outermost
        // Pointers are leftmost (first), arrays/functions are rightmost (last)
        // We need to split: pointers wrap inside-out, arrays/functions wrap outside-in
        let mut pointers: Vec<bool> = Vec::new(); // true = const pointee
        for derived in &declarator.derived {
            match &derived.node {
                DerivedDeclarator::Pointer(qualifiers) => {
                    let is_const = qualifiers.iter().any(|q| {
                        matches!(
                            q.node,
                            PointerQualifier::TypeQualifier(ref tq)
                                if tq.node == TypeQualifier::Const
                        )
                    });
                    pointers.push(is_const);
                }
                DerivedDeclarator::Array(arr) => {
                    let size = match &arr.node.size {
                        ArraySize::Unknown | ArraySize::VariableUnknown => None,
                        ArraySize::VariableExpression(expr)
                        | ArraySize::StaticExpression(expr) => {
                            eval_const_expr(&expr.node).map(|v| v as u64)
                        }
                    };
                    ty = BindingType::Array(Box::new(ty), size);
                }
                DerivedDeclarator::Function(fdecl) => {
                    let params = self.extract_parameters(&fdecl.node.parameters);
                    let variadic = fdecl.node.ellipsis == Ellipsis::Some;
                    ty = BindingType::FunctionPointer {
                        return_type: Box::new(ty),
                        parameters: params.iter().map(|p| p.ty.clone()).collect(),
                        variadic,
                    };
                }
                _ => {}
            }
        }

        // In C, `const char *p` means: pointer to const char.
        // The pointer qualifiers in pac attach to the POINTER level, not the pointee.
        // `const char *p` parses as: base=char, specifier has Const, pointer has no qualifier.
        // `char * const p` parses as: base=char, pointer has Const qualifier.
        //
        // For Rust FFI we care about pointee constness:
        // - If the BASE type has const (from specifiers), the innermost pointer should be const_pointee=true
        // - If the POINTER has const qualifier, that's pointer-itself-const (irrelevant for Rust)
        //
        // However, pac puts the const on the pointer qualifier list for `const T *`:
        // the const actually qualifies the pointee. The first pointer's qualifiers
        // describe what the pointer points to.
        //
        // Actually in C declaration syntax processed by pac:
        // `const int *p` → specifiers=[Const, Int], derived=[Pointer([])]
        // `int *const p` → specifiers=[Int], derived=[Pointer([Const])]
        // `const int *const p` → specifiers=[Const, Int], derived=[Pointer([Const])]
        //
        // The pointer qualifier Const means the pointer ITSELF is const.
        // The specifier Const means the pointed-to type is const.
        // For `const char *`, const is in the specifiers, NOT in the pointer qualifiers.
        //
        // So we need to check if the base type's specifiers had const.
        // We'll handle that by checking if pointers[0] is const (pointer-self-const)
        // and separately tracking base-const from the caller.
        //
        // For now: pointer qualifiers indicate pointer-self-const which is not
        // what we need. The pointee const comes from the base specifiers.
        // We'll pass that through from the caller. For the simple wrapping here,
        // we just create non-const pointers and let the caller override.
        for _is_ptr_const in &pointers {
            ty = BindingType::Pointer {
                pointee: Box::new(ty),
                const_pointee: false,
            };
        }

        // Handle nested declarator (parenthesized)
        if let DeclaratorKind::Declarator(inner) = &declarator.kind.node {
            ty = self.apply_derived_type(ty, &inner.node);
        }

        ty
    }

    fn resolve_function_parts(
        &self,
        declarator: &Declarator,
        base_return_type: BindingType,
    ) -> (BindingType, Vec<ParameterBinding>, bool) {
        let mut return_type = base_return_type;
        let mut params = Vec::new();
        let mut variadic = false;

        // Apply pointer derivations to return type, find the function declarator
        for derived in &declarator.derived {
            match &derived.node {
                DerivedDeclarator::Pointer(_) => {
                    return_type = BindingType::Pointer {
                        pointee: Box::new(return_type),
                        const_pointee: false,
                    };
                }
                DerivedDeclarator::Function(fdecl) => {
                    params = self.extract_parameters(&fdecl.node.parameters);
                    variadic = fdecl.node.ellipsis == Ellipsis::Some;
                }
                _ => {}
            }
        }

        (return_type, params, variadic)
    }

    fn extract_parameters(
        &self,
        params: &[Node<ParameterDeclaration>],
    ) -> Vec<ParameterBinding> {
        // Handle `void` parameter (single unnamed void param means no params)
        if params.len() == 1 {
            let p = &params[0].node;
            if p.declarator.is_none() {
                let base = self.resolve_base_type_from_param_specifiers(&p.specifiers);
                if base == BindingType::Void {
                    return Vec::new();
                }
            }
        }

        params
            .iter()
            .map(|p| {
                let name = p
                    .node
                    .declarator
                    .as_ref()
                    .and_then(|d| self.declarator_name(&d.node));
                let base = self.resolve_base_type_from_param_specifiers(&p.node.specifiers);
                let base_is_const = self.has_const_qualifier(&p.node.specifiers);
                let mut ty = match &p.node.declarator {
                    Some(d) => self.apply_derived_type(base, &d.node),
                    None => base,
                };
                if base_is_const {
                    ty = self.mark_innermost_pointer_const(ty);
                }
                ParameterBinding { name, ty }
            })
            .collect()
    }

    fn resolve_base_type_from_param_specifiers(
        &self,
        specifiers: &[Node<DeclarationSpecifier>],
    ) -> BindingType {
        let type_specs: Vec<_> = specifiers
            .iter()
            .filter_map(|s| match &s.node {
                DeclarationSpecifier::TypeSpecifier(ts) => Some(ts),
                _ => None,
            })
            .collect();
        self.resolve_base_type_from_type_specs(&type_specs)
    }

    fn is_function_declarator(&self, declarator: &Declarator) -> bool {
        declarator.derived.iter().any(|d| {
            matches!(d.node, DerivedDeclarator::Function(_))
        })
    }

    fn declarator_name(&self, declarator: &Declarator) -> Option<String> {
        match &declarator.kind.node {
            DeclaratorKind::Identifier(id) => Some(id.node.name.clone()),
            DeclaratorKind::Declarator(inner) => self.declarator_name(&inner.node),
            DeclaratorKind::Abstract => None,
        }
    }

    fn emit_specifier_diagnostics(
        &mut self,
        specifiers: &[Node<DeclarationSpecifier>],
        item_name: &str,
        offset: usize,
    ) {
        for spec in specifiers {
            match &spec.node {
                DeclarationSpecifier::TypeSpecifier(ts) => match &ts.node {
                    TypeSpecifier::Complex => {
                        self.diagnostics.push(
                            Diagnostic::warning(
                                DiagnosticKind::DeclarationUnsupported,
                                format!("_Complex type not supported on '{}'", item_name),
                            )
                            .with_item(item_name)
                            .with_location(None, offset),
                        );
                    }
                    TypeSpecifier::TypeOf(_) => {
                        self.diagnostics.push(
                            Diagnostic::warning(
                                DiagnosticKind::DeclarationPartial,
                                format!("typeof not resolved on '{}'", item_name),
                            )
                            .with_item(item_name)
                            .with_location(None, offset),
                        );
                    }
                    _ => {}
                },
                DeclarationSpecifier::TypeQualifier(tq) => {
                    if tq.node == TypeQualifier::Atomic {
                        self.diagnostics.push(
                            Diagnostic::warning(
                                DiagnosticKind::DeclarationPartial,
                                format!("_Atomic qualifier ignored on '{}'", item_name),
                            )
                            .with_item(item_name)
                            .with_location(None, offset),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn emit_derived_diagnostics(
        &mut self,
        declarator: &Declarator,
        item_name: &str,
        offset: usize,
    ) {
        for derived in &declarator.derived {
            match &derived.node {
                DerivedDeclarator::KRFunction(_) => {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            DiagnosticKind::DeclarationUnsupported,
                            format!("K&R function declaration on '{}'", item_name),
                        )
                        .with_item(item_name)
                        .with_location(None, offset),
                    );
                }
                DerivedDeclarator::Block(_) => {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            DiagnosticKind::DeclarationUnsupported,
                            format!("block pointer not supported on '{}'", item_name),
                        )
                        .with_item(item_name)
                        .with_location(None, offset),
                    );
                }
                _ => {}
            }
        }
    }

    fn emit_extension_diagnostics(
        &mut self,
        extensions: &[Node<pac::ast::Extension>],
        item_name: &str,
        offset: usize,
    ) {
        if extensions.is_empty() {
            return;
        }
        let attr_names: Vec<String> = extensions
            .iter()
            .filter_map(|e| match &e.node {
                pac::ast::Extension::Attribute(a) => Some(a.name.node.clone()),
                pac::ast::Extension::AsmLabel(_) => Some("asm_label".into()),
                pac::ast::Extension::AvailabilityAttribute(_) => {
                    Some("availability".into())
                }
            })
            .collect();
        if !attr_names.is_empty() {
            self.diagnostics.push(
                Diagnostic::warning(
                    DiagnosticKind::DeclarationPartial,
                    format!("attributes ignored: {}", attr_names.join(", ")),
                )
                .with_item(item_name)
                .with_location(None, offset),
            );
        }
    }
}

/// Best-effort constant expression evaluation for enum values and array sizes.
fn eval_const_expr(expr: &Expression) -> Option<i128> {
    match expr {
        Expression::Constant(c) => match &c.node {
            Constant::Integer(i) => {
                let s = i.number.as_ref();
                let val = match i.base {
                    IntegerBase::Decimal => i128::from_str_radix(s, 10).ok(),
                    IntegerBase::Octal => i128::from_str_radix(s, 8).ok(),
                    IntegerBase::Hexadecimal => i128::from_str_radix(s, 16).ok(),
                    IntegerBase::Binary => i128::from_str_radix(s, 2).ok(),
                };
                val
            }
            _ => None,
        },
        Expression::UnaryOperator(u) => {
            let inner = eval_const_expr(&u.node.operand.node)?;
            match u.node.operator.node {
                UnaryOperator::Minus => Some(-inner),
                UnaryOperator::Plus => Some(inner),
                UnaryOperator::Complement => Some(!inner),
                _ => None,
            }
        }
        Expression::BinaryOperator(b) => {
            let lhs = eval_const_expr(&b.node.lhs.node)?;
            let rhs = eval_const_expr(&b.node.rhs.node)?;
            match b.node.operator.node {
                BinaryOperator::Plus => Some(lhs + rhs),
                BinaryOperator::Minus => Some(lhs - rhs),
                BinaryOperator::Multiply => Some(lhs * rhs),
                BinaryOperator::Divide if rhs != 0 => Some(lhs / rhs),
                BinaryOperator::Modulo if rhs != 0 => Some(lhs % rhs),
                BinaryOperator::ShiftLeft => Some(lhs << (rhs as u32)),
                BinaryOperator::ShiftRight => Some(lhs >> (rhs as u32)),
                BinaryOperator::BitwiseAnd => Some(lhs & rhs),
                BinaryOperator::BitwiseOr => Some(lhs | rhs),
                BinaryOperator::BitwiseXor => Some(lhs ^ rhs),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Parse a C source string and extract bindings.
pub fn extract_from_source(source: &str) -> Result<BindingPackage, String> {
    let unit = pac::parse::translation_unit(source, pac::driver::Flavor::GnuC11)
        .map_err(|e| format!("parse error at line {}:{}: {:?}", e.line, e.column, e.expected))?;

    let extractor = Extractor::new();
    let (items, diagnostics) = extractor.extract(&unit);

    Ok(BindingPackage {
        source_path: None,
        items,
        diagnostics,
    })
}

/// Extract bindings from an already-parsed translation unit.
pub fn extract_from_translation_unit(
    unit: &TranslationUnit,
    source_path: Option<String>,
) -> BindingPackage {
    let extractor = Extractor::new();
    let (items, diagnostics) = extractor.extract(unit);

    BindingPackage {
        source_path,
        items,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(source: &str) -> BindingPackage {
        extract_from_source(source).expect("parse failed")
    }

    #[test]
    fn extract_typedef_int() {
        let pkg = extract("typedef int my_int;");
        assert_eq!(pkg.items.len(), 1);
        match &pkg.items[0] {
            BindingItem::TypeAlias(ta) => {
                assert_eq!(ta.name, "my_int");
                assert_eq!(ta.target, BindingType::Int);
            }
            other => panic!("expected TypeAlias, got {:?}", other),
        }
    }

    #[test]
    fn extract_typedef_pointer() {
        let pkg = extract("typedef void *handle_t;");
        assert_eq!(pkg.items.len(), 1);
        match &pkg.items[0] {
            BindingItem::TypeAlias(ta) => {
                assert_eq!(ta.name, "handle_t");
                assert_eq!(ta.target, BindingType::ptr(BindingType::Void));
            }
            other => panic!("expected TypeAlias, got {:?}", other),
        }
    }

    #[test]
    fn extract_typedef_unsigned_long() {
        let pkg = extract("typedef unsigned long size_t;");
        match &pkg.items[0] {
            BindingItem::TypeAlias(ta) => {
                assert_eq!(ta.name, "size_t");
                assert_eq!(ta.target, BindingType::ULong);
            }
            other => panic!("expected TypeAlias, got {:?}", other),
        }
    }

    #[test]
    fn extract_extern_function() {
        let pkg = extract("extern int puts(const char *s);");
        assert_eq!(pkg.items.len(), 1);
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.name, "puts");
                assert_eq!(f.return_type, BindingType::Int);
                assert!(!f.variadic);
                assert_eq!(f.parameters.len(), 1);
                assert_eq!(f.parameters[0].name.as_deref(), Some("s"));
                assert_eq!(
                    f.parameters[0].ty,
                    BindingType::const_ptr(BindingType::Char)
                );
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_variadic_function() {
        let pkg = extract("int printf(const char *fmt, ...);");
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.name, "printf");
                assert!(f.variadic);
                assert_eq!(f.parameters.len(), 1);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_void_function() {
        let pkg = extract("void abort(void);");
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.name, "abort");
                assert_eq!(f.return_type, BindingType::Void);
                assert!(f.parameters.is_empty());
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_enum() {
        let pkg = extract("enum color { RED, GREEN = 5, BLUE };");
        // Should have an Enum item
        let enums: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Enum(e) => Some(e),
            _ => None,
        }).collect();
        assert_eq!(enums.len(), 1);
        let e = &enums[0];
        assert_eq!(e.name.as_deref(), Some("color"));
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[0].name, "RED");
        assert_eq!(e.variants[0].value, None);
        assert_eq!(e.variants[1].name, "GREEN");
        assert_eq!(e.variants[1].value, Some(5));
        assert_eq!(e.variants[2].name, "BLUE");
    }

    #[test]
    fn extract_struct_with_fields() {
        let pkg = extract("struct point { int x; int y; };");
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.kind, RecordKind::Struct);
        assert_eq!(r.name.as_deref(), Some("point"));
        assert!(!r.is_opaque());
        let fields = r.fields.as_ref().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name.as_deref(), Some("x"));
        assert_eq!(fields[0].ty, BindingType::Int);
    }

    #[test]
    fn extract_opaque_struct() {
        let pkg = extract("struct FILE;");
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1);
        assert!(records[0].is_opaque());
        assert_eq!(records[0].name.as_deref(), Some("FILE"));
    }

    #[test]
    fn extract_union() {
        let pkg = extract("union data { int i; float f; };");
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, RecordKind::Union);
        assert_eq!(records[0].name.as_deref(), Some("data"));
    }

    #[test]
    fn extract_function_returning_pointer() {
        let pkg = extract("void *malloc(unsigned long size);");
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.name, "malloc");
                assert_eq!(
                    f.return_type,
                    BindingType::ptr(BindingType::Void)
                );
                assert_eq!(f.parameters.len(), 1);
                assert_eq!(f.parameters[0].ty, BindingType::ULong);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_extern_variable() {
        let pkg = extract("extern int errno;");
        match &pkg.items[0] {
            BindingItem::Variable(v) => {
                assert_eq!(v.name, "errno");
                assert_eq!(v.ty, BindingType::Int);
            }
            other => panic!("expected Variable, got {:?}", other),
        }
    }

    #[test]
    fn extract_typedef_struct() {
        let pkg = extract("typedef struct point { int x; int y; } point_t;");
        // Should produce a Record and a TypeAlias
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        let aliases: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::TypeAlias(ta) => Some(ta),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1);
        assert_eq!(aliases.len(), 1);
        assert_eq!(records[0].name.as_deref(), Some("point"));
        assert_eq!(aliases[0].name, "point_t");
        assert_eq!(aliases[0].target, BindingType::RecordRef("point".into()));
    }

    #[test]
    fn extract_multiple_declarations() {
        let src = r#"
            typedef unsigned long size_t;
            void *malloc(size_t size);
            void free(void *ptr);
            struct FILE;
            extern int errno;
        "#;
        let pkg = extract(src);
        assert_eq!(pkg.items.len(), 5);
    }

    #[test]
    fn extract_function_pointer_typedef() {
        let pkg = extract("typedef void (*handler_t)(int sig);");
        match &pkg.items[0] {
            BindingItem::TypeAlias(ta) => {
                assert_eq!(ta.name, "handler_t");
                match &ta.target {
                    BindingType::Pointer { pointee: inner, .. } => match inner.as_ref() {
                        BindingType::FunctionPointer { return_type, parameters, variadic } => {
                            assert_eq!(**return_type, BindingType::Void);
                            assert_eq!(parameters.len(), 1);
                            assert_eq!(parameters[0], BindingType::Int);
                            assert!(!variadic);
                        }
                        other => panic!("expected FunctionPointer inside Pointer, got {:?}", other),
                    },
                    other => panic!("expected Pointer, got {:?}", other),
                }
            }
            other => panic!("expected TypeAlias, got {:?}", other),
        }
    }

    #[test]
    fn extract_const_char_pointer_param() {
        let pkg = extract("int puts(const char *s);");
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.parameters[0].ty, BindingType::const_ptr(BindingType::Char));
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_mutable_pointer_param() {
        let pkg = extract("void fill(int *buf);");
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.parameters[0].ty, BindingType::ptr(BindingType::Int));
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_const_void_pointer_return() {
        let pkg = extract("const void *memchr(const void *s, int c, unsigned long n);");
        match &pkg.items[0] {
            BindingItem::Function(f) => {
                assert_eq!(f.return_type, BindingType::const_ptr(BindingType::Void));
                assert_eq!(f.parameters[0].ty, BindingType::const_ptr(BindingType::Void));
                assert_eq!(f.parameters[1].ty, BindingType::Int);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn extract_const_field_pointer() {
        let pkg = extract("struct s { const char *name; int *data; };");
        let records: Vec<_> = pkg.items.iter().filter_map(|i| match i {
            BindingItem::Record(r) => Some(r),
            _ => None,
        }).collect();
        let fields = records[0].fields.as_ref().unwrap();
        assert_eq!(fields[0].ty, BindingType::const_ptr(BindingType::Char));
        assert_eq!(fields[1].ty, BindingType::ptr(BindingType::Int));
    }

    #[test]
    fn diag_bitfield_warning() {
        let pkg = extract("struct s { int x : 3; int y; };");
        assert_eq!(pkg.diagnostics.len(), 1);
        assert_eq!(pkg.diagnostics[0].kind, DiagnosticKind::DeclarationPartial);
        assert!(pkg.diagnostics[0].message.contains("bitfield"));
        assert_eq!(pkg.diagnostics[0].item_name.as_deref(), Some("x"));
    }

    #[test]
    fn diag_static_function_unsupported() {
        let pkg = extract("static int helper(void) { return 0; }");
        // Static function should not produce a BindingItem::Function
        let funcs: Vec<_> = pkg
            .items
            .iter()
            .filter(|i| matches!(i, BindingItem::Function(_)))
            .collect();
        assert!(funcs.is_empty());
        assert_eq!(pkg.diagnostics.len(), 1);
        assert_eq!(
            pkg.diagnostics[0].kind,
            DiagnosticKind::DeclarationUnsupported
        );
        assert!(pkg.diagnostics[0].message.contains("static"));
    }

    #[test]
    fn diag_atomic_qualifier() {
        let pkg = extract("_Atomic int counter(void);");
        let atomics: Vec<_> = pkg
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("_Atomic"))
            .collect();
        assert_eq!(atomics.len(), 1);
        assert_eq!(atomics[0].kind, DiagnosticKind::DeclarationPartial);
    }

    #[test]
    fn diag_diagnostics_count_by_kind() {
        let pkg = extract("struct s { int a : 1; int b : 2; };");
        let counts = pkg.diagnostics_count_by_kind();
        assert_eq!(counts.get("DeclarationPartial"), Some(&2));
    }

    #[test]
    fn ir_determinism() {
        let src = r#"
            typedef int int32_t;
            enum status { OK = 0, ERR = 1 };
            struct point { int x; int y; };
            void *malloc(unsigned long size);
        "#;
        let pkg1 = extract(src);
        let pkg2 = extract(src);
        let json1 = serde_json::to_string(&pkg1).unwrap();
        let json2 = serde_json::to_string(&pkg2).unwrap();
        assert_eq!(json1, json2);
    }
}
