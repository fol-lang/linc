use crate::ir::*;

pub struct RustEmitter {
    output: String,
}

impl RustEmitter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    pub fn emit(mut self, package: &BindingPackage) -> String {
        // Collect items by category
        let mut aliases = Vec::new();
        let mut enums = Vec::new();
        let mut records = Vec::new();
        let mut functions = Vec::new();
        let mut variables = Vec::new();

        for item in &package.items {
            match item {
                BindingItem::TypeAlias(a) => aliases.push(a),
                BindingItem::Enum(e) => enums.push(e),
                BindingItem::Record(r) => records.push(r),
                BindingItem::Function(f) => functions.push(f),
                BindingItem::Variable(v) => variables.push(v),
                BindingItem::Unsupported(_) => {}
            }
        }

        // Emit type aliases
        for alias in &aliases {
            self.emit_type_alias(alias);
        }

        // Emit enums
        for e in &enums {
            self.emit_enum(e);
        }

        // Emit records
        for r in &records {
            self.emit_record(r);
        }

        // Emit extern block with functions and variables
        if !functions.is_empty() || !variables.is_empty() {
            self.output.push_str("extern \"C\" {\n");
            for f in functions.iter().filter(|f| f.calling_convention == CallingConvention::C) {
                self.emit_function(f);
            }
            for v in &variables {
                self.emit_variable(v);
            }
            self.output.push_str("}\n");

            for f in functions.iter().filter(|f| f.calling_convention != CallingConvention::C) {
                self.emit_function_block(f);
            }
        }

        self.output
    }

    fn emit_type_alias(&mut self, alias: &TypeAliasBinding) {
        let target = self.render_type(&alias.target);
        self.output
            .push_str(&format!("pub type {} = {};\n", alias.name, target));
    }

    fn emit_enum(&mut self, e: &EnumBinding) {
        let name = match &e.name {
            Some(n) => n.clone(),
            None => return, // Skip anonymous enums without a name
        };

        // Emit as a newtype over c_int with associated constants
        self.output
            .push_str(&format!("pub type {} = ::core::ffi::c_int;\n", name));
        for variant in &e.variants {
            match variant.value {
                Some(val) => {
                    self.output.push_str(&format!(
                        "pub const {}: {} = {};\n",
                        variant.name, name, val
                    ));
                }
                None => {
                    self.output.push_str(&format!(
                        "// pub const {}: {} = <unknown>;\n",
                        variant.name, name
                    ));
                }
            }
        }
    }

    fn emit_record(&mut self, r: &RecordBinding) {
        let name = match &r.name {
            Some(n) => n.clone(),
            None => return,
        };

        if r.is_opaque() {
            self.output.push_str(&format!(
                "#[repr(C)]\npub struct {} {{ _opaque: [u8; 0] }}\n",
                name
            ));
            return;
        }

        let keyword = match r.kind {
            RecordKind::Struct => "#[repr(C)]\npub struct",
            RecordKind::Union => "#[repr(C)]\npub union",
        };

        let fields = match r.fields.as_ref() {
            Some(f) => f,
            None => return, // shouldn't happen after is_opaque check
        };
        self.output.push_str(&format!("{} {} {{\n", keyword, name));
        for (i, field) in fields.iter().enumerate() {
            let default_name = format!("__bindgen_anon_{}", i);
            let fname = field.name.as_deref().unwrap_or(&default_name);
            let ftype = self.render_type(&field.ty);
            self.output
                .push_str(&format!("    pub {}: {},\n", fname, ftype));
        }
        self.output.push_str("}\n");
    }

    fn emit_function(&mut self, f: &FunctionBinding) {
        self.emit_function_signature("    ", f);
    }

    fn emit_function_block(&mut self, f: &FunctionBinding) {
        self.output.push_str(&format!(
            "extern \"{}\" {{\n",
            self.render_calling_convention(&f.calling_convention)
        ));
        self.emit_function_signature("    ", f);
        self.output.push_str("}\n");
    }

    fn emit_function_signature(&mut self, indent: &str, f: &FunctionBinding) {
        let ret = if f.return_type.is_void() {
            String::new()
        } else {
            format!(" -> {}", self.render_type(&f.return_type))
        };

        let mut params: Vec<String> = f
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let default_name = format!("arg{}", i);
                let name = p.name.as_deref().unwrap_or(&default_name);
                let ty = self.render_type(&p.ty);
                format!("{}: {}", name, ty)
            })
            .collect();

        if f.variadic {
            params.push("...".into());
        }

        self.output.push_str(&format!(
            "{indent}pub fn {}({}){};\n",
            f.name,
            params.join(", "),
            ret
        ));
    }

    fn emit_variable(&mut self, v: &VariableBinding) {
        let ty = self.render_type(&v.ty);
        self.output
            .push_str(&format!("    pub static {}: {};\n", v.name, ty));
    }

    fn render_type(&self, ty: &BindingType) -> String {
        match ty {
            BindingType::Void => "::core::ffi::c_void".into(),
            BindingType::Bool => "bool".into(),
            BindingType::Char => "::core::ffi::c_char".into(),
            BindingType::SChar => "::core::ffi::c_schar".into(),
            BindingType::UChar => "::core::ffi::c_uchar".into(),
            BindingType::Short => "::core::ffi::c_short".into(),
            BindingType::UShort => "::core::ffi::c_ushort".into(),
            BindingType::Int => "::core::ffi::c_int".into(),
            BindingType::UInt => "::core::ffi::c_uint".into(),
            BindingType::Long => "::core::ffi::c_long".into(),
            BindingType::ULong => "::core::ffi::c_ulong".into(),
            BindingType::LongLong => "::core::ffi::c_longlong".into(),
            BindingType::ULongLong => "::core::ffi::c_ulonglong".into(),
            BindingType::Float => "f32".into(),
            BindingType::Double => "f64".into(),
            BindingType::LongDouble => "[u8; 16]".into(), // long double (platform-dependent)
            BindingType::Pointer {
                pointee,
                const_pointee,
                ..
            } => {
                let mutability = if *const_pointee { "*const" } else { "*mut" };
                if pointee.is_void() {
                    format!("{} ::core::ffi::c_void", mutability)
                } else {
                    format!("{} {}", mutability, self.render_type(pointee))
                }
            }
            BindingType::Array(inner, Some(size)) => {
                format!("[{}; {}]", self.render_type(inner), size)
            }
            BindingType::Array(inner, None) => {
                // Flexible array member — represent as pointer
                format!("*mut {} /* flexible array */", self.render_type(inner))
            }
            BindingType::Qualified { ty, .. } => self.render_type(ty),
            BindingType::FunctionPointer {
                return_type,
                parameters,
                variadic,
            } => {
                let params: Vec<String> =
                    parameters.iter().map(|p| self.render_type(p)).collect();
                let mut param_str = params.join(", ");
                if *variadic {
                    if !param_str.is_empty() {
                        param_str.push_str(", ");
                    }
                    param_str.push_str("...");
                }
                let ret = if return_type.is_void() {
                    String::new()
                } else {
                    format!(" -> {}", self.render_type(return_type))
                };
                format!("unsafe extern \"C\" fn({}){}", param_str, ret)
            }
            BindingType::TypedefRef(name) => name.clone(),
            BindingType::RecordRef(name) => name.clone(),
            BindingType::EnumRef(name) => name.clone(),
            BindingType::Opaque(name) => format!("() /* opaque: {} */", name),
        }
    }

    fn render_calling_convention(&self, calling_convention: &CallingConvention) -> &'static str {
        match calling_convention {
            CallingConvention::C | CallingConvention::Cdecl | CallingConvention::Unknown(_) => "C",
            CallingConvention::Stdcall => "stdcall",
            CallingConvention::Fastcall => "fastcall",
            CallingConvention::Vectorcall => "vectorcall",
            CallingConvention::Thiscall => "thiscall",
        }
    }
}

pub fn emit_rust_ffi(package: &BindingPackage) -> String {
    RustEmitter::new().emit(package)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_source;

    fn gen(c_src: &str) -> String {
        let pkg = extract_from_source(c_src).unwrap();
        emit_rust_ffi(&pkg)
    }

    #[test]
    fn emit_simple_function() {
        let out = gen("void foo(void);");
        assert!(out.contains("extern \"C\""));
        assert!(out.contains("pub fn foo()"));
    }

    #[test]
    fn emit_function_with_params() {
        let out = gen("int add(int a, int b);");
        assert!(out.contains("pub fn add(a: ::core::ffi::c_int, b: ::core::ffi::c_int) -> ::core::ffi::c_int"));
    }

    #[test]
    fn emit_variadic_function() {
        let out = gen("int printf(const char *fmt, ...);");
        assert!(out.contains("pub fn printf(fmt: *const ::core::ffi::c_char, ...)"));
    }

    #[test]
    fn emit_typedef() {
        let out = gen("typedef unsigned long size_t;");
        assert!(out.contains("pub type size_t = ::core::ffi::c_ulong;"));
    }

    #[test]
    fn emit_opaque_struct() {
        let out = gen("struct FILE;");
        assert!(out.contains("#[repr(C)]"));
        assert!(out.contains("pub struct FILE { _opaque: [u8; 0] }"));
    }

    #[test]
    fn emit_struct_with_fields() {
        let out = gen("struct point { int x; int y; };");
        assert!(out.contains("#[repr(C)]"));
        assert!(out.contains("pub struct point"));
        assert!(out.contains("pub x: ::core::ffi::c_int"));
        assert!(out.contains("pub y: ::core::ffi::c_int"));
    }

    #[test]
    fn emit_union() {
        let out = gen("union data { int i; float f; };");
        assert!(out.contains("#[repr(C)]"));
        assert!(out.contains("pub union data"));
    }

    #[test]
    fn emit_enum_as_constants() {
        let out = gen("enum color { RED = 0, GREEN = 1, BLUE = 2 };");
        assert!(out.contains("pub type color = ::core::ffi::c_int;"));
        assert!(out.contains("pub const RED: color = 0;"));
        assert!(out.contains("pub const GREEN: color = 1;"));
        assert!(out.contains("pub const BLUE: color = 2;"));
    }

    #[test]
    fn emit_extern_variable() {
        let out = gen("extern int errno;");
        assert!(out.contains("pub static errno: ::core::ffi::c_int;"));
    }

    #[test]
    fn emit_pointer_return() {
        let out = gen("void *malloc(unsigned long size);");
        assert!(out.contains("pub fn malloc(size: ::core::ffi::c_ulong) -> *mut ::core::ffi::c_void"));
    }

    #[test]
    fn emit_combined() {
        let src = r#"
            typedef unsigned long size_t;
            struct FILE;
            void *malloc(size_t size);
            void free(void *ptr);
        "#;
        let out = gen(src);
        assert!(out.contains("pub type size_t"));
        assert!(out.contains("pub struct FILE"));
        assert!(out.contains("pub fn malloc"));
        assert!(out.contains("pub fn free"));
    }

    #[test]
    fn emit_deterministic() {
        let src = "typedef int x; void foo(x a);";
        let out1 = gen(src);
        let out2 = gen(src);
        assert_eq!(out1, out2);
    }

    #[test]
    fn emit_function_pointer_typedef() {
        let out = gen("typedef void (*handler_t)(int sig);");
        assert!(out.contains("pub type handler_t"));
    }

    #[test]
    fn emit_const_pointer_param() {
        let out = gen("int puts(const char *s);");
        assert!(out.contains("s: *const ::core::ffi::c_char"));
    }

    #[test]
    fn emit_mut_pointer_param() {
        let out = gen("void fill(int *buf);");
        assert!(out.contains("buf: *mut ::core::ffi::c_int"));
    }

    #[test]
    fn emit_const_void_pointer_return() {
        let out = gen("const void *find(void);");
        assert!(out.contains("*const ::core::ffi::c_void"));
    }

    #[test]
    fn emit_long_double_as_byte_array() {
        let out = gen("long double ld_func(long double x);");
        assert!(out.contains("[u8; 16]"));
        assert!(!out.contains("f64"));
    }

    #[test]
    fn emit_function_pointer_without_option() {
        let out = gen("typedef void (*cb_t)(int);");
        assert!(out.contains("unsafe extern \"C\" fn("));
        assert!(!out.contains("Option<"));
    }

    #[test]
    fn emit_anonymous_field_bindgen_name() {
        // A struct with an anonymous inner struct produces unnamed fields
        let pkg = crate::ir::BindingPackage {
            source_path: None,
            items: vec![crate::ir::BindingItem::Record(crate::ir::RecordBinding {
                kind: crate::ir::RecordKind::Struct,
                name: Some("s".into()),
                fields: Some(vec![
                    crate::ir::FieldBinding {
                        name: None,
                        ty: crate::ir::BindingType::Int,
                        bit_width: None,
                        layout: None,
                    },
                    crate::ir::FieldBinding {
                        name: Some("x".into()),
                        ty: crate::ir::BindingType::Int,
                        bit_width: None,
                        layout: None,
                    },
                ]),
                representation: None,
                abi_confidence: None,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..crate::ir::BindingPackage::new()
        };
        let out = emit_rust_ffi(&pkg);
        assert!(out.contains("__bindgen_anon_0"));
        assert!(out.contains("pub x:"));
    }

    #[test]
    fn emit_memcpy_signature() {
        let out = gen("void *memcpy(void *dest, const void *src, unsigned long n);");
        assert!(out.contains("dest: *mut ::core::ffi::c_void"));
        assert!(out.contains("src: *const ::core::ffi::c_void"));
        assert!(out.contains("-> *mut ::core::ffi::c_void"));
    }

    #[test]
    fn emit_unsized_array_as_flexible_member() {
        let pkg = crate::ir::BindingPackage {
            source_path: None,
            items: vec![crate::ir::BindingItem::Record(crate::ir::RecordBinding {
                kind: crate::ir::RecordKind::Struct,
                name: Some("buf".into()),
                fields: Some(vec![
                    crate::ir::FieldBinding {
                        name: Some("len".into()),
                        ty: crate::ir::BindingType::Int,
                        bit_width: None,
                        layout: None,
                    },
                    crate::ir::FieldBinding {
                        name: Some("data".into()),
                        ty: crate::ir::BindingType::Array(
                            Box::new(crate::ir::BindingType::UChar),
                            None,
                        ),
                        bit_width: None,
                        layout: None,
                    },
                ]),
                representation: None,
                abi_confidence: None,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..crate::ir::BindingPackage::new()
        };
        let out = emit_rust_ffi(&pkg);
        assert!(out.contains("flexible array"));
    }

    #[test]
    fn emit_opaque_type_valid_rust() {
        let pkg = crate::ir::BindingPackage {
            source_path: None,
            items: vec![crate::ir::BindingItem::Function(crate::ir::FunctionBinding {
                name: "get".into(),
                calling_convention: crate::ir::CallingConvention::C,
                parameters: Vec::new(),
                return_type: crate::ir::BindingType::Opaque("_Complex".into()),
                variadic: false,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..crate::ir::BindingPackage::new()
        };
        let out = emit_rust_ffi(&pkg);
        assert!(out.contains("opaque: _Complex"));
        assert!(out.contains("()"));
    }
}
